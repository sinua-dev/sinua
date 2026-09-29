package dev.sinua.gemini

import android.util.Log
import dev.sinua.voice.AgentState
import dev.sinua.voice.AndroidPcmAudioDevice
import dev.sinua.voice.CredentialException
import dev.sinua.voice.CredentialSource
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
 * Credentials: the shared contract ([CredentialSource], docs/audio-pipeline.md).
 * Only an `auth_tokens/…` ephemeral token is accepted, minted by your backend
 * (`mintGeminiLiveCredential` in `@sinua/voice/server`, or `npx @sinua/voice
 * dev-proxy`), which locks the model, voice and instructions. It travels as
 * `Authorization: Token …`, never in the URL. With [CredentialSource.url] or a
 * provider, every reconnect resumes the session with a **new** token (single-use
 * by default). A raw API key is refused.
 *
 * Not verified against the live API yet (docs/audio-pipeline.md).
 */
class GeminiLiveVoiceSource(
    /** Where tokens come from: [CredentialSource.url], `.provider { … }`, or `.fixed(…)` for one session. */
    private val credentials: CredentialSource,
    model: String = GeminiLiveSession.DEFAULT_MODEL,
    /** Deprecated: set instructions when your backend mints the token, which locks them. */
    instructions: String? = null,
    /** Override the Google URL (a relay your backend runs, or tests); the token's header is still added. */
    private val endpointOverride: GeminiLiveSession.Endpoint? = null,
    device: PcmAudioDevice = AndroidPcmAudioDevice(),
    private val socketFactory: LiveSocketFactory = OkHttpLiveSocketFactory(),
    private val main: MainDispatcher = LooperMainDispatcher(),
) : VoiceSource {
    /** One fixed token (a pasted `auth_tokens/…`, single session). */
    @JvmOverloads
    constructor(
        credential: String,
        model: String = GeminiLiveSession.DEFAULT_MODEL,
        instructions: String? = null,
        endpoint: GeminiLiveSession.Endpoint? = null,
        device: PcmAudioDevice = AndroidPcmAudioDevice(),
        socketFactory: LiveSocketFactory = OkHttpLiveSocketFactory(),
        main: MainDispatcher = LooperMainDispatcher(),
    ) : this(CredentialSource.fixed(credential), model, instructions, endpoint, device, socketFactory, main)

    init {
        if (instructions != null) {
            log(
                "GeminiLiveVoiceSource: `instructions` is deprecated -- set them when your backend mints the token " +
                    "(mintGeminiLiveCredential from @sinua/voice/server), which locks them.",
            )
        }
    }

    private var endpoint: GeminiLiveSession.Endpoint? = null
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

    override val supportsMute: Boolean get() = true
    override val reportsConnection: Boolean get() = true
    private var connectionCb: ((Boolean) -> Unit)? = null
    private var sessionUp = false

    override fun onConnectionChange(cb: (Boolean) -> Unit) {
        connectionCb = cb
    }

    private fun setSessionUp(up: Boolean) {
        if (up == sessionUp) return
        sessionUp = up
        connectionCb?.invoke(up)
    }

    @Volatile private var muted = false

    /** Muted, the mic chunks go out zeroed (the server's turn detection keeps its timing); the session stays up. */
    override fun setMuted(muted: Boolean) {
        this.muted = muted
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
    override fun onError(cb: (Throwable) -> Unit) {
        errorCb = cb
    }

    /** Call on the main thread. */
    override fun connect() {
        if (wantConnected) return
        // Before the socket, before the mic: a missing or refused credential must
        // not open a device or a connection. A fixed value answers synchronously,
        // so its refusal is thrown from here; a provider/URL answers on main.
        var syncError: Throwable? = null
        var sync = true
        refreshEndpoint { err ->
            if (err != null) {
                if (sync) syncError = err else errorCb?.invoke(err)
                return@refreshEndpoint
            }
            wantConnected = true
            session.reset()
            session.connecting()
            // Authenticate first; the audio (and the mic) only after setupComplete.
            openSocket(
                onReady = { startAudio() },
                onFail = { e ->
                    teardown()
                    errorCb?.invoke(e)
                },
            )
        }
        sync = false
        syncError?.let { throw it }
    }

    /** A token for the next socket; an `auth_tokens/…` name is the only accepted shape. */
    private fun refreshEndpoint(done: (Throwable?) -> Unit) {
        credentials.resolve("GeminiLiveVoiceSource", main) { r ->
            val err = r.exceptionOrNull() ?: runCatching {
                val token = r.getOrThrow().credential
                InsecureCredential.check(
                    "GeminiLiveVoiceSource",
                    InsecureCredential.isGeminiEphemeral(token),
                    InsecureCredential.GEMINI_SHAPE,
                )
                val auth = GeminiLiveSession.endpoint(token)
                endpoint =
                    endpointOverride?.let { GeminiLiveSession.Endpoint(it.url, it.headers + auth.headers) } ?: auth
            }.exceptionOrNull()
            done(err)
        }
    }

    private fun startAudio() {
        try {
            graph.start(GeminiLiveSession.INPUT_RATE, GeminiLiveSession.OUTPUT_RATE) { samples ->
                micSocket?.send(GeminiLiveSession.micMessage(if (muted) FloatArray(samples.size) else samples))
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
        setSessionUp(true)
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
        val e = endpoint ?: return onFail(IllegalStateException("GeminiLiveVoiceSource: no credential"))
        opened = socketFactory.open(
            e.url,
            e.headers,
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
        val fail: (Throwable) -> Unit = { err ->
            log("Gemini Live reconnect $n/$RECONNECT_ATTEMPTS after $reason failed: ${err.message}")
            if (n < RECONNECT_ATTEMPTS && !(err is CredentialException && err.fatal)) {
                main.postDelayed({ attempt(n + 1, reason) }, RECONNECT_DELAY_MS)
            } else {
                reconnecting = false
                log("Gemini Live: gave up reconnecting")
                teardown()
                errorCb?.invoke(err)
            }
        }
        val open = { openSocket(onReady = { reconnecting = false }, onFail = fail) }
        // A token is single-use by default: resume with a new one when the caller
        // can mint it (a pasted token is tried as is).
        if (!credentials.canRefresh) return open()
        refreshEndpoint { err ->
            if (!wantConnected) return@refreshEndpoint
            if (err != null) fail(err) else open()
        }
    }

    // --- tick / teardown ---

    private fun startTicker() {
        if (ticking || !wantConnected) return
        ticking = true
        main.post(ticker)
    }

    private fun teardown() {
        setSessionUp(false)
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
