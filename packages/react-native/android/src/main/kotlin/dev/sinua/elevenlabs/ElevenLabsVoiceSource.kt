package dev.sinua.elevenlabs

import android.util.Log
import dev.sinua.voice.AgentState
import dev.sinua.voice.AndroidPcmAudioDevice
import dev.sinua.voice.ElevenLabsSession
import dev.sinua.voice.LiveSocket
import dev.sinua.voice.LiveSocketFactory
import dev.sinua.voice.LooperMainDispatcher
import dev.sinua.voice.MainDispatcher
import dev.sinua.voice.Pcm
import dev.sinua.voice.PcmAudioDevice
import dev.sinua.voice.PcmAudioGraph
import dev.sinua.voice.VoiceMetrics
import dev.sinua.voice.VoiceSource
import dev.sinua.websocket.OkHttpLiveSocketFactory
import org.json.JSONObject

/**
 * `VoiceSource` for ElevenLabs Conversational AI (the Agents platform) over its
 * raw WebSocket -- the native mirror of the Web `ElevenLabsVoiceSource`
 * (packages/voice/src/ElevenLabsVoiceSource.ts) and iOS `SinuaElevenLabs`.
 * Protocol and state rules live in `ElevenLabsSession`, the playback gate in
 * `PcmAudioGraph` (both dev.sinua.voice, JVM-tested); this class wires them to
 * OkHttp and an audio device. Callbacks arrive on the main thread.
 *
 * Why not ElevenLabs' own Android SDK: it carries voice over LiveKit and keeps
 * its Room private, so there's no agent audio to analyse; the WebSocket protocol
 * is still documented for audio (docs/audio-pipeline.md).
 *
 * Order: socket + `conversation_initiation_metadata` first, then the audio at
 * the negotiated rates -- a bad agent id or expired signed URL fails without
 * opening the mic. `connect()` is synchronous (`VoiceSource`): it returns after
 * opening the socket; failures after that (setup, a non-PCM input format,
 * starting the audio, e.g. no RECORD_AUDIO) go to `onError`, with the state
 * already `idle`. No reconnect: a conversation isn't resumable.
 *
 * Credentials: a public agent's `agent_id`, or a `wss://…` signed URL minted by
 * your backend for a private agent.
 *
 * Not verified against the live service yet (docs/audio-pipeline.md).
 */
class ElevenLabsVoiceSource(
    credential: String,
    private val overrides: JSONObject? = null,
    /** Override the URL (a relay, or tests); the credential is ignored then. */
    endpoint: String? = null,
    device: PcmAudioDevice = AndroidPcmAudioDevice(),
    private val socketFactory: LiveSocketFactory = OkHttpLiveSocketFactory(),
    private val main: MainDispatcher = LooperMainDispatcher(),
    private val clock: () -> Double = { System.nanoTime() / 1e9 },
) : VoiceSource {
    private val url: String? =
        endpoint ?: credential.trim().takeIf { it.isNotEmpty() }?.let { ElevenLabsSession.endpoint(it) }
    private val session = ElevenLabsSession()
    private val graph = PcmAudioGraph(device)

    // Main-thread state.
    private var socket: LiveSocket? = null

    @Volatile private var micSocket: LiveSocket? = null // read on the capture thread
    private var metadataPending = false
    private var wantConnected = false
    private var ticking = false
    private var metricsCb: ((VoiceMetrics) -> Unit)? = null
    private var errorCb: ((Throwable) -> Unit)? = null

    private val timeout =
        Runnable {
            if (metadataPending) {
                fail(
                    IllegalStateException("ElevenLabs did not send conversation_initiation_metadata within 15s"),
                )
            }
        }

    private val ticker = object : Runnable {
        override fun run() {
            if (!ticking) return
            graph.read()?.let { metricsCb?.invoke(it) }
            session.tick(graph.playbackState(), clock())
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

    /** Failures after `connect()` returned (the state is already back to idle). */
    fun onError(cb: (Throwable) -> Unit) {
        errorCb = cb
    }

    /** Call on the main thread. */
    override fun connect() {
        val url = checkNotNull(url) { "ElevenLabsVoiceSource: an agent id or signed URL is required" }
        if (wantConnected) return
        wantConnected = true
        session.connecting(clock())
        metadataPending = true
        lateinit var opened: LiveSocket
        opened = socketFactory.open(
            url,
            emptyMap(),
            listOf(ElevenLabsSession.SUBPROTOCOL),
            onText = { text -> main.post { if (socket === opened) onText(text) } },
            onClose = { err -> main.post { if (socket === opened) onSocketClosed(err) } },
        )
        socket = opened
        opened.send(ElevenLabsSession.initMessage(overrides))
        main.postDelayed(timeout, METADATA_TIMEOUT_MS)
    }

    override fun disconnect() {
        teardown()
    }

    private fun onText(text: String) {
        for (action in session.handle(text, graph.playbackState(), clock())) apply(action)
    }

    private fun apply(action: ElevenLabsSession.Action) {
        when (action) {
            is ElevenLabsSession.Action.Metadata -> onMetadata(action.input, action.output)
            is ElevenLabsSession.Action.Send -> socket?.send(action.text)
            is ElevenLabsSession.Action.Enqueue -> graph.enqueue(action.samples, action.rate)
            is ElevenLabsSession.Action.ClearPlayback -> graph.clearPlayback(action.fade)
        }
    }

    private fun onMetadata(input: Pcm.AudioFormat, output: Pcm.AudioFormat) {
        if (!metadataPending) return
        metadataPending = false
        main.remove(timeout)
        if (input.codec != Pcm.AudioFormat.Codec.PCM) {
            fail(
                IllegalStateException(
                    "ElevenLabs agent expects ${input.codec.name.lowercase()}_${input.rate} input; configure a pcm_* input format",
                ),
            )
            return
        }
        try {
            graph.start(input.rate, output.rate) { samples -> micSocket?.send(ElevenLabsSession.micMessage(samples)) }
        } catch (e: Throwable) {
            fail(e)
            return
        }
        session.graphStarted(output, clock()).forEach { apply(it) }
        micSocket = socket
        if (!ticking) {
            ticking = true
            main.post(ticker)
        }
    }

    private fun onSocketClosed(err: Throwable?) {
        if (metadataPending) {
            fail(err ?: IllegalStateException("ElevenLabs closed during setup"))
            return
        }
        if (!wantConnected) return
        // Not resumable: 1000 means the agent ended the conversation; anything else is logged.
        log("ElevenLabs socket closed: ${err?.message ?: "normal closure"}")
        teardown()
    }

    private fun fail(err: Throwable) {
        teardown()
        errorCb?.invoke(err)
    }

    private fun teardown() {
        wantConnected = false
        metadataPending = false
        ticking = false
        main.remove(ticker)
        main.remove(timeout)
        micSocket = null
        val s = socket
        socket = null
        s?.close()
        graph.stop()
        session.stopped(clock())
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
        const val METADATA_TIMEOUT_MS = 15_000L
        private const val TAG = "ElevenLabs"
    }
}
