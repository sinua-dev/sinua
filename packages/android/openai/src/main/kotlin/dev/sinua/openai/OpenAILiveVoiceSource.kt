package dev.sinua.openai

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.util.Log
import dev.sinua.voice.AgentState
import dev.sinua.voice.CredentialException
import dev.sinua.voice.CredentialSource
import dev.sinua.voice.InsecureCredential
import dev.sinua.voice.MainDispatcher
import dev.sinua.voice.OpenAILiveSession
import dev.sinua.voice.OpenAILiveSignaling
import dev.sinua.voice.OpenAIRealtimeSignaling
import dev.sinua.voice.PcmTap
import dev.sinua.voice.RealtimeReconnect
import dev.sinua.voice.VoiceMetrics
import dev.sinua.voice.VoiceSource
import dev.sinua.voice.isMicPermissionGranted
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.delay
import kotlinx.coroutines.launch
import kotlinx.coroutines.suspendCancellableCoroutine
import kotlinx.coroutines.withTimeout
import livekit.org.webrtc.AudioTrack
import livekit.org.webrtc.AudioTrackSink
import livekit.org.webrtc.DataChannel
import livekit.org.webrtc.IceCandidate
import livekit.org.webrtc.MediaConstraints
import livekit.org.webrtc.MediaStream
import livekit.org.webrtc.PeerConnection
import livekit.org.webrtc.PeerConnectionFactory
import livekit.org.webrtc.RtpReceiver
import livekit.org.webrtc.RtpTransceiver
import livekit.org.webrtc.SessionDescription
import java.nio.ByteBuffer
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException

/**
 * `VoiceSource` for OpenAI's GPT-Live (`gpt-live-1`) over WebRTC -- the native mirror of
 * the Web `OpenAILiveVoiceSource` (packages/voice/src/OpenAILiveVoiceSource.ts) and iOS
 * `SinuaOpenAI`, on the same LiveKit WebRTC build as [OpenAIRealtimeVoiceSource].
 *
 * GPT-Live has no client credential: your server opens the session
 * (`POST /v1/live/sessions` with its project key). This source gathers ICE, POSTs
 * `{ "sdp": … }` as JSON to your [sessionUrl] (with `Authorization: Bearer` when you give
 * it a credential -- your own token, never `sk-…`), and takes OpenAI's 201 JSON back
 * unchanged (or the bare SDP). The session is live at `session.started`; the client never
 * sends `session.start`. Model, voice, instructions, delegation and prior conversation are
 * set by your server.
 *
 * State comes from [OpenAILiveSession] (dev.sinua.voice, JVM-tested against
 * spec/openai-live-cases.json): the model's audio level is `speaking`, an open backend
 * delegation is `thinking`, user speech that stops the model is a barge-in.
 *
 * `disconnect()` goes idle at once, then sends `session.close` and keeps the call until
 * `session.closed` (up to 5 s) so the final usage is confirmed. `expired` /
 * `connection_lost`, or a call that drops without `session.closed`, is replaced by a new
 * session (up to 3 attempts, a fresh credential each); `close_requested`,
 * `remote_hangup` and `content` end in idle. `connect()` is synchronous: failures after
 * it returns go to `onError`, with the state already `idle`.
 *
 * `warp`: WARP's libwebrtc field trials only (DTLS 1.3 / SNAP / SPED). OpenAI documents no
 * `dcid` for GPT-Live, so there's no pre-negotiated channel here; experimental.
 *
 * **Not exercised at runtime by any test** (a peer connection opens the real mic and
 * speakers). Compile-verified; the I/O-free rules are JVM-tested.
 */
class OpenAILiveVoiceSource(
    context: Context,
    /** Your endpoint that opens the GPT-Live session (never `api.openai.com`). */
    private val sessionUrl: String,
    /** Your own token for it, fresh per session with [CredentialSource.url] / `.provider`; null for none. */
    private val credentials: CredentialSource? = null,
    private val warp: Boolean = false,
    private val reconnect: Boolean = true,
    private val http: OpenAIHttp = OpenAIHttp(),
) : VoiceSource {
    private val appContext = context.applicationContext
    private val session = OpenAILiveSession()
    private val tap = PcmTap()
    private val handler = Handler(Looper.getMainLooper())
    private val mainDispatcher = object : MainDispatcher {
        override fun post(r: Runnable) {
            handler.post(r)
        }
        override fun postDelayed(r: Runnable, delayMs: Long) {
            handler.postDelayed(r, delayMs)
        }
        override fun remove(r: Runnable) = handler.removeCallbacks(r)
    }

    // Main-thread state.
    private var scope: CoroutineScope? = null
    private var factory: PeerConnectionFactory? = null
    private var pc: PeerConnection? = null
    private var channel: DataChannel? = null
    private var remoteTrack: AudioTrack? = null
    private var gathered: CompletableDeferred<Unit>? = null
    private var started: CompletableDeferred<Unit>? = null
    private var closing: Runnable? = null
    private var reconnecting = false
    private var live = false
    private var metricsCb: ((VoiceMetrics) -> Unit)? = null
    private var errorCb: ((Throwable) -> Unit)? = null
    private var connectionCb: ((Boolean) -> Unit)? = null
    private var sessionUp = false
    private var muted = false
    private var micTrack: AudioTrack? = null

    init {
        session.onClosed = { closed(it) }
    }

    /** `session.started`'s id for the current call, once live (e.g. for your server's sideband). */
    val sessionId: String? get() = session.sessionId

    /** WebRTC thread: copy the model's PCM into the tap. */
    private val sink = AudioTrackSink { data: ByteBuffer, bits: Int, _: Int, channels: Int, frames: Int, _: Long ->
        tap.sink.onPcm(data, bits, channels, frames)
    }

    private val ticker = object : Runnable {
        override fun run() {
            if (!live) return
            if (remoteTrack != null) {
                val m = tap.read()
                metricsCb?.invoke(m)
                session.tick(m.level, now())
            } else {
                session.tick(0.0, now()) // delegation timeouts still run
            }
            handler.postDelayed(this, (1000 / UPDATE_HZ).toLong())
        }
    }

    override val supportsMute: Boolean get() = true
    override val reportsConnection: Boolean get() = true

    override fun onConnectionChange(cb: (Boolean) -> Unit) {
        connectionCb = cb
    }

    private fun setSessionUp(up: Boolean) {
        if (up == sessionUp) return
        sessionUp = up
        connectionCb?.invoke(up)
    }

    /** Muted, the mic track is disabled: WebRTC sends silence and the session stays up. */
    override fun setMuted(muted: Boolean) {
        val apply = {
            this.muted = muted
            micTrack?.setEnabled(!muted)
        }
        if (Looper.myLooper() == Looper.getMainLooper()) apply() else handler.post { apply() }
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
    override fun onError(cb: (Throwable) -> Unit) {
        errorCb = cb
    }

    /** Your token for this (re)connect, or null without a credential source. */
    private suspend fun resolveToken(): String? {
        val source = credentials ?: return null
        val token = suspendCancellableCoroutine { cont ->
            source.resolve("OpenAILiveVoiceSource", mainDispatcher) { r ->
                r.fold({ cont.resume(it.credential) }, { cont.resumeWithException(it) })
            }
        }
        InsecureCredential.checkOpenAILive(sessionUrl, token)
        return token
    }

    /** Call on the main thread. */
    override fun connect() {
        finishClose() // a graceful close still draining from a previous disconnect()
        if (scope != null) return
        val s = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
        scope = s
        session.connecting()
        s.launch {
            try {
                InsecureCredential.checkOpenAILive(sessionUrl, null)
                val token = resolveToken() // auth first: a bad credential never opens the mic
                if (!isMicPermissionGranted(appContext)) throw SecurityException("RECORD_AUDIO not granted")
                call(token)
                live = true
                handler.post(ticker)
                setSessionUp(true)
            } catch (e: CancellationException) {
                throw e
            } catch (e: Throwable) {
                teardown()
                errorCb?.invoke(e)
            }
        }
    }

    /**
     * Idle at once; then `session.close`, and the call is kept (mic silent, playback off)
     * until `session.closed` confirms the final usage, or 5 s pass.
     */
    override fun disconnect() {
        if (Looper.myLooper() != Looper.getMainLooper()) {
            handler.post { disconnect() }
            return
        }
        val graceful = live && session.isStarted && channel?.state() == DataChannel.State.OPEN
        live = false
        reconnecting = false
        handler.removeCallbacks(ticker)
        session.stopped()
        setSessionUp(false)
        scope?.cancel()
        scope = null
        if (!graceful) return teardown()
        micTrack?.setEnabled(false)
        remoteTrack?.setEnabled(false)
        metricsCb?.invoke(VoiceMetrics.SILENT)
        send("""{"type":"session.close"}""")
        val timeout = Runnable {
            log("GPT-Live: no session.closed within 5s; final usage unconfirmed")
            finishClose()
        }
        closing = timeout
        handler.postDelayed(timeout, CLOSE_TIMEOUT_MS)
    }

    /** Ends a graceful close (answered, timed out, or cut short by a new connect). Main thread. */
    private fun finishClose() {
        val timeout = closing ?: return
        handler.removeCallbacks(timeout)
        closing = null
        teardown()
    }

    // --- the call (one GPT-Live session), main thread ---

    private suspend fun call(token: String?) {
        val f = factory ?: createPeerConnectionFactory(appContext, warp).also { factory = it }
        val peer = f.createPeerConnection(
            PeerConnection.RTCConfiguration(emptyList()).apply {
                sdpSemantics = PeerConnection.SdpSemantics.UNIFIED_PLAN
            },
            Observer(),
        ) ?: throw IllegalStateException("could not create a peer connection")
        pc = peer
        val constraints = MediaConstraints()
        val mic = f.createAudioTrack("mic", f.createAudioSource(constraints))
        mic.setEnabled(!muted)
        micTrack = mic
        peer.addTrack(mic, listOf("mic"))
        // Created before the offer (OpenAI's sequence).
        val dc = peer.createDataChannel("oai-events", DataChannel.Init())
        channel = dc
        dc.registerObserver(ChannelObserver(dc))
        val gathering = CompletableDeferred<Unit>().also { gathered = it }
        val startedSignal = CompletableDeferred<Unit>().also { started = it }

        val offer = peer.awaitOffer(constraints)
        peer.awaitSetLocal(offer)
        // GPT-Live takes one complete offer (no trickle ICE).
        if (peer.iceGatheringState() != PeerConnection.IceGatheringState.COMPLETE) {
            withTimeout(ICE_GATHERING_TIMEOUT_MS) { gathering.await() }
        }
        val sdp = peer.localDescription?.description ?: offer.description
        val (status, body) = http.send(OpenAILiveSignaling.sessionRequest(sdp, token, sessionUrl))
        val answer = OpenAILiveSignaling.answer(status, body)
        if (pc !== peer) throw CancellationException("disconnected")
        peer.awaitSetRemote(SessionDescription(SessionDescription.Type.ANSWER, answer.sdp))
        if (!session.isStarted) withTimeout(STARTED_TIMEOUT_MS) { startedSignal.await() }
    }

    private fun send(text: String) {
        channel?.send(DataChannel.Buffer(ByteBuffer.wrap(text.toByteArray()), false))
    }

    private fun closeCall() {
        remoteTrack?.removeSink(sink)
        remoteTrack = null
        tap.reset()
        gathered?.cancel()
        gathered = null
        started?.cancel()
        started = null
        channel?.unregisterObserver()
        channel?.close()
        channel = null
        micTrack = null
        pc?.close()
        pc = null
    }

    /** `session.closed` from the server. Main thread. */
    private fun closed(reason: String) {
        started?.completeExceptionally(
            CredentialException("OpenAILiveVoiceSource: the session closed before it started ($reason)", true),
        )
        if (closing != null) return finishClose()
        if (!live) return
        if (reason in RECONNECTING_REASONS) {
            dropped("session closed ($reason)")
        } else {
            log("GPT-Live session closed ($reason)")
            teardown()
        }
    }

    /** Main thread. */
    private fun dropped(reason: String) {
        if (closing != null) return finishClose()
        if (!live || reconnecting) return
        val s = scope ?: return
        reconnecting = true
        val fatal = session.fatalCode
        closeCall()
        session.connecting()
        metricsCb?.invoke(VoiceMetrics.SILENT) // go quiet instead of freezing
        s.launch {
            if (reconnect && fatal == null) {
                for (attempt in 1..RealtimeReconnect.DEFAULT_ATTEMPTS) {
                    delay(RealtimeReconnect.delayMs(attempt).toLong())
                    try {
                        call(resolveToken())
                        reconnecting = false
                        return@launch
                    } catch (e: CancellationException) {
                        throw e
                    } catch (e: OpenAIRealtimeSignaling.SignalingException.Fatal) {
                        break
                    } catch (e: OpenAIRealtimeSignaling.SignalingException.Malformed) {
                        break
                    } catch (e: CredentialException) {
                        if (e.fatal) break
                        log("GPT-Live reconnect $attempt after $reason failed: ${e.message}")
                    } catch (e: Throwable) {
                        log("GPT-Live reconnect $attempt after $reason failed: ${e.message}")
                    }
                    closeCall()
                }
            }
            reconnecting = false
            teardown()
            errorCb?.invoke(IllegalStateException("GPT-Live: gave up reconnecting after $reason"))
        }
    }

    private fun teardown() {
        setSessionUp(false)
        live = false
        handler.removeCallbacks(ticker)
        closing?.let { handler.removeCallbacks(it) }
        closing = null
        closeCall()
        factory?.dispose()
        factory = null
        val s = scope
        scope = null
        session.stopped()
        s?.cancel()
    }

    private fun log(msg: String) {
        try {
            Log.w(TAG, msg)
        } catch (_: RuntimeException) {
            // android.util.Log is a stub under plain JVM tests.
        }
    }

    // --- WebRTC observers (WebRTC threads -> main) ---

    private inner class Observer : PeerConnection.Observer {
        override fun onSignalingChange(state: PeerConnection.SignalingState?) {}
        override fun onIceConnectionChange(state: PeerConnection.IceConnectionState?) {
            if (state == PeerConnection.IceConnectionState.FAILED ||
                state == PeerConnection.IceConnectionState.CLOSED
            ) {
                handler.post { dropped("ice $state") }
            }
        }
        override fun onIceConnectionReceivingChange(receiving: Boolean) {}
        override fun onIceGatheringChange(state: PeerConnection.IceGatheringState?) {
            if (state == PeerConnection.IceGatheringState.COMPLETE) handler.post { gathered?.complete(Unit) }
        }
        override fun onIceCandidate(candidate: IceCandidate?) {}
        override fun onIceCandidatesRemoved(candidates: Array<out IceCandidate>?) {}
        override fun onAddStream(stream: MediaStream?) {}
        override fun onRemoveStream(stream: MediaStream?) {}
        override fun onDataChannel(dc: DataChannel?) {}
        override fun onRenegotiationNeeded() {}
        override fun onAddTrack(receiver: RtpReceiver?, streams: Array<out MediaStream>?) {
            val track = receiver?.track() as? AudioTrack ?: return
            handler.post {
                if (remoteTrack === track) return@post
                remoteTrack?.removeSink(sink)
                tap.reset()
                remoteTrack = track
                track.addSink(sink)
            }
        }
        override fun onTrack(transceiver: RtpTransceiver?) {}
    }

    private inner class ChannelObserver(private val dc: DataChannel) : DataChannel.Observer {
        override fun onBufferedAmountChange(previousAmount: Long) {}
        override fun onStateChange() {
            val state = dc.state()
            handler.post { if (dc === channel && state == DataChannel.State.CLOSED) dropped("data channel closed") }
        }
        override fun onMessage(buffer: DataChannel.Buffer) {
            val bytes = ByteArray(buffer.data.remaining()).also { buffer.data.get(it) }
            val text = String(bytes, Charsets.UTF_8)
            handler.post {
                if (dc !== channel) return@post
                session.handle(text, now())
                if (session.isStarted) started?.complete(Unit)
            }
        }
    }

    companion object {
        const val UPDATE_HZ = 30.0
        const val ICE_GATHERING_TIMEOUT_MS = 10_000L
        const val STARTED_TIMEOUT_MS = 15_000L
        const val CLOSE_TIMEOUT_MS = 5_000L
        private val RECONNECTING_REASONS = setOf("expired", "connection_lost")
        private const val TAG = "OpenAILive"

        private fun now(): Double = SystemClock.elapsedRealtime().toDouble()
    }
}
