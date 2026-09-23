package dev.sinua.gemini

import android.util.Log
import dev.sinua.voice.AgentState
import dev.sinua.voice.AndroidPcmAudioDevice
import dev.sinua.voice.GeminiLiveSession
import dev.sinua.voice.InsecureCredential
import dev.sinua.voice.LiveSocket
import dev.sinua.voice.LiveSocketFactory
import dev.sinua.voice.LooperMainDispatcher
import dev.sinua.voice.MainDispatcher
import dev.sinua.voice.PcmAudioDevice
import dev.sinua.voice.PcmAudioGraph
import dev.sinua.voice.VoiceMetrics
import dev.sinua.voice.VoiceSource
import dev.sinua.websocket.OkHttpLiveSocketFactory

/**
 * `VoiceSource` for Gemini Live over its raw WebSocket -- the native mirror of
 * the Web `GeminiLiveVoiceSource` (packages/voice/src/GeminiLiveVoiceSource.ts)
 * and iOS `SinuaGeminiLive`. Protocol and state rules live in
 * `GeminiLiveSession`, the playback-timeline gate in `PcmAudioGraph` (both
 * dev.sinua.voice, JVM-tested); this class wires them to OkHttp and an audio
 * device. Callbacks arrive on the main thread.
 *
 * Mic PCM16 @ 16 kHz goes up; the model's PCM16 @ 24 kHz is scheduled on the
 * player; `speaking` and the metrics follow what has actually played. Session
 * resumption + reconnect on `goAway` or an unexpected close (3 attempts, 500 ms
 * apart), as on Web.
 *
 * `connect()` is synchronous on Android (`VoiceSource`): it opens the socket in
 * the background and returns. Only after `setupComplete` does it start the
 * audio (mic + player), so a bad or expired credential fails without ever
 * opening the mic. Any failure after `connect()` returned (setup, or starting
 * the audio, e.g. no RECORD_AUDIO) goes to `onError`, with the state already
 * back at `idle`.
 *
 * Credentials: an `auth_tokens/…` ephemeral token minted by your backend
 * (production shape), or -- dev only -- a raw API key. Both travel as request
 * headers, never in the URL; held in memory only, never logged.
 *
 * Not verified against the live API yet (docs/audio-pipeline.md).
 */
class GeminiLiveVoiceSource(
    credential: String,
    model: String = GeminiLiveSession.DEFAULT_MODEL,
    instructions: String? = null,
    /**
     * Opt in to a raw, long-lived API key. Without it `connect()` refuses one
     * before the socket or the mic ([InsecureCredential]). Local demos only.
     */
    private val allowInsecureApiKey: Boolean = false,
    /** Override the Google endpoint (a relay your backend runs, or tests). */
    endpoint: GeminiLiveSession.Endpoint? = null,
    device: PcmAudioDevice = AndroidPcmAudioDevice(),
    private val socketFactory: LiveSocketFactory = OkHttpLiveSocketFactory(),
    private val main: MainDispatcher = LooperMainDispatcher(),
) : VoiceSource {
    private val hasCredential = credential.isNotBlank()
    private val credentialIsEphemeral = InsecureCredential.isGeminiEphemeral(credential)
    private val endpoint = endpoint ?: GeminiLiveSession.endpoint(credential)
    private val session = GeminiLiveSession(model, instructions)
    private val graph = PcmAudioGraph(device)

    // Main-thread state.
    private var socket: LiveSocket? = null

    @Volatile private var micSocket: LiveSocket? = null // read on the capture thread
    private var setupPending: SetupWait? = null
    private var wantConnected = false
    private var reconnecting = false
    private var audioStarted = false
    private val pendingAudio = mutableListOf<Pair<FloatArray, Int>>()
    private var ticking = false
    private var metricsCb: ((VoiceMetrics) -> Unit)? = null
    private var errorCb: ((Throwable) -> Unit)? = null

    private class SetupWait(val onReady: () -> Unit, val onFail: (Throwable) -> Unit, val timeout: Runnable)

    private val ticker = object : Runnable {
        override fun run() {
            if (!ticking) return
            graph.read()?.let { metricsCb?.invoke(it) }
            if (!reconnecting) session.tick(graph.playbackState())
            main.postDelayed(this, (1000 / UPDATE_HZ).toLong())
        }
    }

    override fun onMetrics(cb: (VoiceMetrics) -> Unit) {
        metricsCb = cb
    }

    override fun onStateChange(cb: (AgentState) -> Unit) {
        session.onState = cb
    }

    override fun onInterrupt(cb: () -> Unit) {
        session.onInterrupt = cb
    }

    /** Setup / reconnect failures after `connect()` returned (the state is already back to idle). */
    fun onError(cb: (Throwable) -> Unit) {
        errorCb = cb
    }

    /** Call on the main thread. */
    override fun connect() {
        check(hasCredential) { "GeminiLiveVoiceSource: a credential is required" }
        // Before the socket, before the mic: a refused credential must not open
        // a device or a connection.
        InsecureCredential.check(
            vendor = "GeminiLiveVoiceSource",
            isEphemeral = credentialIsEphemeral,
            allowInsecureApiKey = allowInsecureApiKey,
            ephemeralShape = InsecureCredential.GEMINI_SHAPE,
            mintHint = InsecureCredential.GEMINI_MINT_HINT,
        )
        if (wantConnected) return
        wantConnected = true
        session.reset()
        session.connecting()
        // Authenticate first; the audio (and the mic) only after setupComplete.
        openSocket(
            onReady = { startAudio() },
            onFail = { err ->
                teardown()
                errorCb?.invoke(err)
            },
        )
    }

    private fun startAudio() {
        try {
            graph.start(GeminiLiveSession.INPUT_RATE, GeminiLiveSession.OUTPUT_RATE) { samples ->
                micSocket?.send(GeminiLiveSession.micMessage(samples))
            }
        } catch (e: Throwable) {
            teardown()
            errorCb?.invoke(e)
            return
        }
        audioStarted = true
        pendingAudio.forEach { (samples, rate) -> graph.enqueue(samples, rate) }
        pendingAudio.clear()
        micSocket = socket
        startTicker()
    }

    override fun disconnect() {
        teardown()
    }

    // --- socket (main thread) ---

    private fun openSocket(onReady: () -> Unit, onFail: (Throwable) -> Unit) {
        if (!wantConnected) return
        lateinit var opened: LiveSocket
        val timeout = Runnable {
            if (socket ===
                opened
            ) {
                failSetup(IllegalStateException("Gemini Live setup did not complete within 15s"))
            }
        }
        setupPending = SetupWait(onReady, onFail, timeout)
        opened = socketFactory.open(
            endpoint.url,
            endpoint.headers,
            emptyList(),
            onText = { text -> main.post { if (socket === opened) onText(text) } },
            onClose = { err -> main.post { if (socket === opened) onSocketClosed(err) } },
        )
        socket = opened
        opened.send(session.setupMessage())
        main.postDelayed(timeout, SETUP_TIMEOUT_MS)
    }

    private fun onText(text: String) {
        for (action in session.handle(text, graph.playbackState())) {
            when (action) {
                GeminiLiveSession.Action.SetupComplete -> {
                    if (audioStarted) micSocket = socket // a reconnect; the first connect sets it in startAudio
                    val wait = setupPending ?: continue
                    setupPending = null
                    main.remove(wait.timeout)
                    wait.onReady()
                }

                // Model audio between setupComplete and the audio starting is held, not dropped.
                is GeminiLiveSession.Action.Enqueue ->
                    if (audioStarted) {
                        graph.enqueue(action.samples, action.rate)
                    } else {
                        pendingAudio +=
                            action.samples to action.rate
                    }

                is GeminiLiveSession.Action.ClearPlayback -> {
                    pendingAudio.clear()
                    graph.clearPlayback(action.fade)
                }

                is GeminiLiveSession.Action.Reconnect -> reconnect(action.reason)

                is GeminiLiveSession.Action.ServerError -> log("Gemini Live error message: ${action.message}")
            }
        }
    }

    private fun onSocketClosed(err: Throwable?) {
        if (setupPending != null) {
            failSetup(err ?: IllegalStateException("Gemini Live closed during setup"))
            return
        }
        if (wantConnected) reconnect("close: ${err?.message ?: "closed"}")
    }

    private fun failSetup(err: Throwable) {
        val wait = setupPending ?: return
        setupPending = null
        main.remove(wait.timeout)
        closeSocket()
        wait.onFail(err)
    }

    private fun closeSocket() {
        micSocket = null
        val s = socket
        socket = null
        s?.close()
    }

    private fun reconnect(reason: String) {
        if (reconnecting || !wantConnected) return
        reconnecting = true
        closeSocket()
        graph.clearPlayback(false)
        session.connecting()
        attempt(1, reason)
    }

    private fun attempt(n: Int, reason: String) {
        if (!wantConnected) {
            reconnecting = false
            return
        }
        openSocket(
            onReady = { reconnecting = false },
            onFail = { err ->
                log("Gemini Live reconnect $n/$RECONNECT_ATTEMPTS after $reason failed: ${err.message}")
                if (n < RECONNECT_ATTEMPTS) {
                    main.postDelayed({ attempt(n + 1, reason) }, RECONNECT_DELAY_MS)
                } else {
                    reconnecting = false
                    log("Gemini Live: gave up reconnecting")
                    teardown()
                    errorCb?.invoke(err)
                }
            },
        )
    }

    // --- tick / teardown ---

    private fun startTicker() {
        if (ticking || !wantConnected) return
        ticking = true
        main.post(ticker)
    }

    private fun teardown() {
        wantConnected = false
        reconnecting = false
        ticking = false
        main.remove(ticker)
        setupPending?.let { main.remove(it.timeout) }
        setupPending = null
        closeSocket()
        audioStarted = false
        pendingAudio.clear()
        graph.stop()
        session.reset() // idle; a fresh connect() starts a fresh session
    }

    private fun log(msg: String) {
        try {
            Log.w(TAG, msg)
        } catch (_: RuntimeException) {
            // android.util.Log is a stub under plain JVM tests.
        }
    }

    companion object {
        const val UPDATE_HZ = 30.0
        private const val TAG = "GeminiLive"
        const val SETUP_TIMEOUT_MS = 15_000L
        const val RECONNECT_ATTEMPTS = 3
        const val RECONNECT_DELAY_MS = 500L
    }
}
