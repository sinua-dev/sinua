package dev.sinua.livekit

import android.content.Context
import android.os.Handler
import android.os.Looper
import dev.sinua.voice.AgentState
import dev.sinua.voice.LiveKitAgentTracker
import dev.sinua.voice.LiveKitParticipantRole
import dev.sinua.voice.VoiceMetrics
import dev.sinua.voice.VoiceSource
import io.livekit.android.LiveKit
import io.livekit.android.events.RoomEvent
import io.livekit.android.events.collect
import io.livekit.android.room.Room
import io.livekit.android.room.participant.Participant
import io.livekit.android.room.participant.RemoteParticipant
import io.livekit.android.room.track.RemoteAudioTrack
import kotlinx.coroutines.CancellationException
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.CoroutineStart
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.cancel
import kotlinx.coroutines.launch
import kotlinx.coroutines.withTimeoutOrNull
import livekit.org.webrtc.AudioTrackSink
import java.nio.ByteBuffer

/**
 * `VoiceSource` for a LiveKit Room with a LiveKit Agents voice agent in it -- the
 * native mirror of the Web `LiveKitVoiceSource` (the Web Studio's audio/
 * LiveKitVoiceSource.ts) and iOS `SinuaLiveKit`. State comes from the agent's
 * `lk.agent.state` attribute, barge-in is inferred as on Web, and the spectrum
 * comes from the agent's remote audio track through WebRTC's `AudioTrackSink`
 * (client-sdk-android v2.28.2 `RemoteAudioTrack.addSink`, int16 PCM on the
 * playout path) into the same `SpectrumAnalyser` -> `AudioAnalysis` path as the
 * mic. All rules live in `LiveKitAgentTracker` (dev.sinua.voice, JVM-tested);
 * this class only translates Room events. Callbacks arrive on the main thread.
 *
 * Two ways in:
 * - `LiveKitVoiceSource(room)` -- attach to the app's Room. Never connects,
 *   publishes, plays or disconnects it; `disconnect()` only stops listening and
 *   removes the audio sink. The Room may connect later.
 * - `LiveKitVoiceSource.owned(context, url, token)` -- own a Room (demo / quick
 *   start): connects, enables the mic (the app must hold RECORD_AUDIO), releases
 *   the Room on teardown. `VoiceSource.connect()` is synchronous on Android, so
 *   the connection runs on the source's scope; a failure goes to `onError` and
 *   the state returns to `idle`.
 *
 * Not verified against a live agent yet (docs/audio-pipeline.md).
 */
class LiveKitVoiceSource private constructor(
    private val room: Room,
    private val ownsRoom: Boolean,
    private val url: String?,
    private val token: String?,
    private val publishMicrophone: Boolean,
) : VoiceSource {
    constructor(room: Room) : this(room, ownsRoom = false, url = null, token = null, publishMicrophone = false)

    private val tracker = LiveKitAgentTracker()
    private val handler = Handler(Looper.getMainLooper())
    private var scope: CoroutineScope? = null
    private var tappedTrack: RemoteAudioTrack? = null
    private var errorCb: ((Throwable) -> Unit)? = null
    private var agentJoined = CompletableDeferred<Unit>()

    /** WebRTC thread: copy channel 0 into the tracker's ring, nothing else. */
    private val sink = object : AudioTrackSink {
        override fun onData(
            audioData: ByteBuffer,
            bitsPerSample: Int,
            sampleRate: Int,
            numberOfChannels: Int,
            numberOfFrames: Int,
            absoluteCaptureTimestampMs: Long,
        ) = tracker.sink.onPcm(audioData, bitsPerSample, numberOfChannels, numberOfFrames)
    }

    private val ticker = object : Runnable {
        override fun run() {
            tracker.tick()
            handler.postDelayed(this, (1000 / UPDATE_HZ).toLong())
        }
    }

    override fun onMetrics(cb: (VoiceMetrics) -> Unit) {
        tracker.onMetrics = cb
    }

    override fun onStateChange(cb: (AgentState) -> Unit) {
        tracker.onState = cb
    }

    override fun onInterrupt(cb: () -> Unit) {
        tracker.onInterrupt = cb
    }

    /** Owned Room only: connection / agent-join failures (the state is already back to idle). */
    fun onError(cb: (Throwable) -> Unit) {
        errorCb = cb
    }

    /** Call on the main thread. */
    override fun connect() {
        if (scope != null) return
        val s = CoroutineScope(SupervisorJob() + Dispatchers.Main.immediate)
        scope = s
        tracker.start()
        // Subscribe before connecting so no event is missed.
        s.launch(start = CoroutineStart.UNDISPATCHED) { room.events.collect { onEvent(it) } }
        if (!ownsRoom) {
            room.remoteParticipants.values.forEach { seen(it) }
            handler.post(ticker)
            return
        }
        s.launch {
            try {
                room.connect(url!!, token!!)
                if (publishMicrophone) room.localParticipant.setMicrophoneEnabled(true)
                room.remoteParticipants.values.forEach { seen(it) }
                handler.post(ticker)
                if (tracker.agentIdentity == null) {
                    val joined = withTimeoutOrNull(AGENT_JOIN_TIMEOUT_MS) { agentJoined.await() }
                    if (joined ==
                        null
                    ) {
                        throw IllegalStateException(
                            "No agent joined the LiveKit room within 20s (is an agent dispatched to this room?)",
                        )
                    }
                }
            } catch (e: Throwable) {
                if (e is CancellationException) throw e
                teardown()
                errorCb?.invoke(e)
            }
        }
    }

    override fun disconnect() {
        if (Looper.myLooper() == Looper.getMainLooper()) teardown() else handler.post { teardown() }
    }

    private fun onEvent(e: RoomEvent) {
        when (e) {
            is RoomEvent.ParticipantConnected -> seen(e.participant)

            is RoomEvent.ParticipantDisconnected -> {
                val id = e.participant.identity?.value ?: return
                if (tracker.participantLeft(id)) untap()
            }

            is RoomEvent.ParticipantAttributesChanged -> {
                val p = e.participant
                val id = p.identity?.value ?: return
                val hadAgent = tracker.agentIdentity != null
                val role = tracker.attributesChanged(
                    id,
                    p.kind == Participant.Kind.AGENT,
                    p.attributes,
                    e.changedAttributes,
                    room.localParticipant.isSpeaking,
                )
                if (!hadAgent) afterAdoption()
                if (role != LiveKitParticipantRole.OTHER && p is RemoteParticipant) scan(p)
            }

            is RoomEvent.TrackSubscribed -> {
                val track = e.track as? RemoteAudioTrack ?: return
                if (seen(e.participant, scan = false) != LiveKitParticipantRole.OTHER) tap(track)
            }

            is RoomEvent.TrackUnsubscribed -> if (e.track === tappedTrack) {
                untap()
                tracker.audioDetached()
            }

            is RoomEvent.Disconnected -> teardown()

            else -> Unit
        }
    }

    private fun seen(p: RemoteParticipant, scan: Boolean = true): LiveKitParticipantRole {
        val id = p.identity?.value ?: return LiveKitParticipantRole.OTHER
        val hadAgent = tracker.agentIdentity != null
        val role = tracker.participantSeen(id, p.kind == Participant.Kind.AGENT, p.attributes)
        if (scan && role != LiveKitParticipantRole.OTHER) scan(p)
        if (!hadAgent) afterAdoption()
        return role
    }

    /** If an agent was just adopted: pick up a worker already publishing for it (the Web adapter's adoptAgent scan). */
    private fun afterAdoption() {
        if (tracker.agentIdentity == null) return
        for (other in room.remoteParticipants.values) {
            val oid = other.identity?.value ?: continue
            if (tracker.role(oid, other.kind == Participant.Kind.AGENT, other.attributes) ==
                LiveKitParticipantRole.AGENT_WORKER
            ) {
                scan(other)
            }
        }
        agentJoined.complete(Unit)
    }

    /** Tracks subscribed before this source was listening never fire TrackSubscribed for it. */
    private fun scan(p: RemoteParticipant) {
        for (pub in p.trackPublications.values) {
            val track = pub.track as? RemoteAudioTrack ?: continue
            tap(track)
            return
        }
    }

    private fun tap(track: RemoteAudioTrack) {
        if (tappedTrack === track) return
        untap()
        tappedTrack = track
        tracker.audioAttached()
        track.addSink(sink)
    }

    private fun untap() {
        tappedTrack?.removeSink(sink)
        tappedTrack = null
    }

    private fun teardown() {
        val s = scope ?: return
        scope = null
        handler.removeCallbacks(ticker)
        untap()
        tracker.stop()
        agentJoined.cancel()
        agentJoined = CompletableDeferred()
        if (ownsRoom) {
            room.disconnect()
            room.release()
        }
        s.cancel()
    }

    companion object {
        const val UPDATE_HZ = 30.0

        /** components-js `useAgent`'s default, as on Web and iOS. */
        const val AGENT_JOIN_TIMEOUT_MS = 20_000L

        /** Owns its Room (`LiveKit.create`), connected by `connect()` and released on `disconnect()`. */
        fun owned(context: Context, url: String, token: String, publishMicrophone: Boolean = true) =
            LiveKitVoiceSource(LiveKit.create(context.applicationContext), true, url, token, publishMicrophone)
    }
}
