package dev.sinua.openai

import android.content.Context
import android.os.Handler
import android.os.Looper
import android.util.Log
import dev.sinua.voice.AgentState
import dev.sinua.voice.CredentialException
import dev.sinua.voice.CredentialSource
import dev.sinua.voice.InsecureCredential
import dev.sinua.voice.MainDispatcher
import dev.sinua.voice.OpenAIRealtimeSession
import dev.sinua.voice.OpenAIRealtimeSignaling
import dev.sinua.voice.PcmTap
import dev.sinua.voice.RealtimeReconnect
import dev.sinua.voice.SinuaCredential
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
import kotlinx.coroutines.runBlocking
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
import livekit.org.webrtc.SdpObserver
import livekit.org.webrtc.SessionDescription
import livekit.org.webrtc.audio.JavaAudioDeviceModule
import java.nio.ByteBuffer
import kotlin.coroutines.resume
import kotlin.coroutines.resumeWithException
import kotlin.coroutines.suspendCoroutine

/**
 * `VoiceSource` for OpenAI's Realtime API over WebRTC -- the native mirror of the
 * Web `OpenAIRealtimeVoiceSource` (packages/voice/src/OpenAIRealtimeVoiceSource.ts) and iOS
 * `SinuaOpenAI`. OpenAI recommends WebRTC for client devices (WebSocket is
 * "from a trusted server").
 *
 * WebRTC is the LiveKit-maintained prefixed build (`livekit.org.webrtc`, the
 * version livekit-android uses): current Chromium milestones, no clash with
 * another WebRTC copy, one binary for an app that also uses LiveKit. Its
 * `JavaAudioDeviceModule` does capture (hardware echo cancellation where
 * available) and playback; the model's remote track is tapped with an
 * `AudioTrackSink` into `PcmTap` (the mic's spectrum path).
 *
 * Event and reconnect rules live in `OpenAIRealtimeSession` / `RealtimeReconnect`
 * / `OpenAIRealtimeSignaling` (dev.sinua.voice, JVM-tested). Callbacks arrive
 * on the main thread.
 *
 * Order: the credential first, then the call (the app must hold RECORD_AUDIO;
 * checked before the mic track exists). `connect()` is synchronous: failures
 * after it returns go to `onError`, with the state already `idle`.
 *
 * Credentials: the shared contract ([CredentialSource]): [CredentialSource.url]
 * (your backend's endpoint), a provider, or one pasted `ek_`. Only an `ek_` is
 * accepted; a raw API key is refused (docs/audio-pipeline.md).
 *
 * Reconnect: a new session with a fresh credential from the source
 * (up to 3 attempts, LiveKit's backoff), then the transcript is replayed; fatal
 * errors give up at once.
 *
 * **Not exercised at runtime by any test**: a peer connection opens the real
 * microphone and speakers (on an emulator, the host's). Compile-verified only;
 * not verified live.
 */
class OpenAIRealtimeVoiceSource(
    context: Context,
    /** Where the `ek_` comes from: [CredentialSource.url] / `.provider { … }` per (re)connect, or `.fixed` once. */
    private val credentials: CredentialSource,
    private val callsUrl: String = OpenAIRealtimeSignaling.CALLS_URL,
    private val http: OpenAIHttp = OpenAIHttp(),
) : VoiceSource {
    /** A fresh `ek_` from your code (e.g. your backend), called again on every reconnect. */
    constructor(
        context: Context,
        credentialProvider: suspend () -> String,
        callsUrl: String = OpenAIRealtimeSignaling.CALLS_URL,
        http: OpenAIHttp = OpenAIHttp(),
    ) : this(
        context,
        CredentialSource.provider { SinuaCredential(runBlocking { credentialProvider() }) },
        callsUrl,
        http,
    )

    /** A pasted `ek_` is single-session: set once it has been used. */
    private var pastedUsed = false

    /** A fresh `ek_` for this (re)connect; anything else is refused (fatal). */
    private suspend fun resolveKey(): String {
        val ek = suspendCancellableCoroutine { cont ->
            credentials.resolve("OpenAIRealtimeVoiceSource", mainDispatcher) { r ->
                r.fold({ cont.resume(it.credential) }, { cont.resumeWithException(it) })
            }
        }
        InsecureCredential.check(
            "OpenAIRealtimeVoiceSource",
            InsecureCredential.isOpenAIEphemeral(ek),
            InsecureCredential.OPENAI_SHAPE,
        )
        if (!credentials.canRefresh) {
            if (pastedUsed) {
                throw CredentialException(
                    "a pasted ek_ is single-session; reconnecting needs credentialUrl or a provider",
                    true,
                )
            }
            pastedUsed = true
        }
        return ek
    }

    private val mainDispatcher = object : MainDispatcher {
        override fun post(r: Runnable) {
            handler.post(r)
        }
        override fun postDelayed(r: Runnable, delayMs: Long) {
            handler.postDelayed(r, delayMs)
        }
        override fun remove(r: Runnable) = handler.removeCallbacks(r)
    }
    private val appContext = context.applicationContext
    private val session = OpenAIRealtimeSession()
    private val tap = PcmTap()
    private val handler = Handler(Looper.getMainLooper())

    // Main-thread state.
    private var scope: CoroutineScope? = null
    private var factory: PeerConnectionFactory? = null
    private var pc: PeerConnection? = null
    private var channel: DataChannel? = null
    private var remoteTrack: AudioTrack? = null
    private var channelOpen: CompletableDeferred<Unit>? = null
    private var reconnecting = false
    private var live = false
    private var metricsCb: ((VoiceMetrics) -> Unit)? = null
    private var errorCb: ((Throwable) -> Unit)? = null

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
                session.tick(m.level)
            }
            handler.postDelayed(this, (1000 / UPDATE_HZ).toLong())
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
    private var muted = false
    private var micTrack: AudioTrack? = null

    /** Muted, the mic track is disabled: WebRTC sends silence and the call stays up (each reconnect's track too). */
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

    /** Call on the main thread. */
    override fun connect() {
        if (scope != null) return
        val s = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
        scope = s
        session.connecting()
        s.launch {
            try {
                val ek = resolveKey() // auth first: a bad credential never opens the mic
                if (!isMicPermissionGranted(appContext)) throw SecurityException("RECORD_AUDIO not granted")
                call(ek)
                session.connected()
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

    override fun disconnect() {
        if (Looper.myLooper() == Looper.getMainLooper()) teardown() else handler.post { teardown() }
    }

    // --- the call (one Realtime session), main thread ---

    private suspend fun call(ek: String) {
        val f = factory ?: createFactory().also { factory = it }
        val observer = Observer()
        val peer = f.createPeerConnection(
            PeerConnection.RTCConfiguration(emptyList()).apply {
                sdpSemantics = PeerConnection.SdpSemantics.UNIFIED_PLAN
            },
            observer,
        ) ?: throw IllegalStateException("could not create a peer connection")
        pc = peer
        val constraints = MediaConstraints()
        val mic = f.createAudioTrack("mic", f.createAudioSource(constraints))
        mic.setEnabled(!muted)
        micTrack = mic
        peer.addTrack(mic, listOf("mic"))
        val dc = peer.createDataChannel("oai-events", DataChannel.Init())
        channel = dc
        val opened = CompletableDeferred<Unit>()
        channelOpen = opened
        dc.registerObserver(ChannelObserver(dc))

        // Posted right after setLocalDescription, as OpenAI's own browser samples do.
        val offer = peer.awaitOffer(constraints)
        peer.awaitSetLocal(offer)
        val (status, body) = http.send(OpenAIRealtimeSignaling.callsRequest(offer.description, ek, callsUrl))
        val answer = OpenAIRealtimeSignaling.answer(status, body)
        if (pc !== peer) throw CancellationException("disconnected")
        peer.awaitSetRemote(SessionDescription(SessionDescription.Type.ANSWER, answer))
        withTimeout(CONNECT_TIMEOUT_MS) { opened.await() }
        // A reconnect: give the new session the conversation so far (empty on the first call).
        session.transcript.replayEvents().forEach { send(it) }
    }

    private fun createFactory(): PeerConnectionFactory {
        PeerConnectionFactory.initialize(
            PeerConnectionFactory.InitializationOptions.builder(appContext).createInitializationOptions(),
        )
        val adm = JavaAudioDeviceModule.builder(appContext)
            .setUseHardwareAcousticEchoCanceler(true)
            .setUseHardwareNoiseSuppressor(true)
            .createAudioDeviceModule()
        return PeerConnectionFactory.builder().setAudioDeviceModule(adm).createPeerConnectionFactory()
    }

    private fun send(text: String) {
        channel?.send(DataChannel.Buffer(ByteBuffer.wrap(text.toByteArray()), false))
    }

    private fun closeCall() {
        remoteTrack?.removeSink(sink)
        remoteTrack = null
        tap.reset()
        channelOpen?.cancel()
        channelOpen = null
        channel?.unregisterObserver()
        channel?.close()
        channel = null
        micTrack = null
        pc?.close()
        pc = null
    }

    private fun dropped(reason: String) {
        if (!live || reconnecting) return
        val s = scope ?: return
        reconnecting = true
        closeCall()
        session.connecting()
        metricsCb?.invoke(VoiceMetrics.SILENT) // go quiet instead of freezing
        s.launch {
            if (session.fatalCode == null) {
                for (attempt in 1..RealtimeReconnect.DEFAULT_ATTEMPTS) {
                    delay(RealtimeReconnect.delayMs(attempt).toLong())
                    try {
                        call(resolveKey())
                        reconnecting = false
                        session.connected()
                        return@launch
                    } catch (e: CancellationException) {
                        throw e
                    } catch (e: OpenAIRealtimeSignaling.SignalingException.Fatal) {
                        break
                    } catch (e: CredentialException) {
                        if (e.fatal) break
                        log("OpenAI Realtime reconnect $attempt after $reason failed: ${e.message}")
                    } catch (e: Throwable) {
                        log("OpenAI Realtime reconnect $attempt after $reason failed: ${e.message}")
                    }
                    closeCall()
                }
            }
            reconnecting = false
            teardown()
            errorCb?.invoke(IllegalStateException("OpenAI Realtime: gave up reconnecting after $reason"))
        }
    }

    private fun teardown() {
        setSessionUp(false)
        live = false
        handler.removeCallbacks(ticker)
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
        override fun onIceGatheringChange(state: PeerConnection.IceGatheringState?) {}
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
            handler.post {
                if (dc !== channel) return@post
                when (state) {
                    DataChannel.State.OPEN -> channelOpen?.complete(Unit)

                    DataChannel.State.CLOSED -> {
                        val waiting = channelOpen
                        if (waiting != null && !waiting.isCompleted) {
                            waiting.completeExceptionally(
                                IllegalStateException("the oai-events channel closed during setup"),
                            )
                        } else {
                            dropped("data channel closed")
                        }
                    }

                    else -> Unit
                }
            }
        }
        override fun onMessage(buffer: DataChannel.Buffer) {
            val bytes = ByteArray(buffer.data.remaining()).also { buffer.data.get(it) }
            val text = String(bytes, Charsets.UTF_8)
            handler.post { if (dc === channel) session.handle(text) }
        }
    }

    companion object {
        const val UPDATE_HZ = 30.0
        const val CONNECT_TIMEOUT_MS = 20_000L
        private const val TAG = "OpenAIRealtime"

        /** Your backend's endpoint, answering `{ credential: "ek_…", expiresAt? }`; asked on every (re)connect. */
        @JvmStatic
        fun withCredentialUrl(context: Context, credentialUrl: String): OpenAIRealtimeVoiceSource =
            OpenAIRealtimeVoiceSource(context, CredentialSource.url(credentialUrl))

        /**
         * One pasted `ek_…` (single session: a drop without a provider ends in `idle`).
         * The session's model, voice and instructions are fixed when your backend mints
         * the `ek_`; there's nothing to set here.
         */
        @JvmStatic
        fun withCredential(context: Context, credential: String): OpenAIRealtimeVoiceSource =
            OpenAIRealtimeVoiceSource(context, CredentialSource.fixed(credential))
    }
}

// SdpObserver -> suspend.
private suspend fun PeerConnection.awaitOffer(c: MediaConstraints): SessionDescription = suspendCoroutine { cont ->
    createOffer(
        object : SdpObserver {
            override fun onCreateSuccess(sdp: SessionDescription) = cont.resume(sdp)
            override fun onCreateFailure(error: String?) =
                cont.resumeWithException(IllegalStateException("createOffer: $error"))
            override fun onSetSuccess() {}
            override fun onSetFailure(error: String?) {}
        },
        c,
    )
}

private suspend fun PeerConnection.awaitSetLocal(sdp: SessionDescription) = suspendCoroutine<Unit> { cont ->
    setLocalDescription(setObserver(cont), sdp)
}

private suspend fun PeerConnection.awaitSetRemote(sdp: SessionDescription) = suspendCoroutine<Unit> { cont ->
    setRemoteDescription(setObserver(cont), sdp)
}

private fun setObserver(cont: kotlin.coroutines.Continuation<Unit>) = object : SdpObserver {
    override fun onCreateSuccess(sdp: SessionDescription?) {}
    override fun onCreateFailure(error: String?) {}
    override fun onSetSuccess() = cont.resume(Unit)
    override fun onSetFailure(error: String?) =
        cont.resumeWithException(IllegalStateException("setDescription: $error"))
}
