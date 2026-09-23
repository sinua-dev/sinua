// The SDK-free half of the native LiveKit `VoiceSource` (the Kotlin glue lives
// in the :sinua-livekit module, so apps that never use LiveKit don't pull
// the SDK). A port of packages/voice/src/livekitAgent.ts plus the state
// machine of the Web `LiveKitVoiceSource`, driven by plain events so it can be
// tested on the JVM. Mirrors packages/ios SinuaVoice/LiveKitAgent.swift.
// Sources: the vendors' public docs and SDK sources.
package dev.sinua.voice

import java.nio.ByteBuffer
import java.nio.ByteOrder

object LiveKitAgent {
    const val AGENT_STATE_ATTRIBUTE = "lk.agent.state"
    const val PUBLISH_ON_BEHALF_ATTRIBUTE = "lk.publish_on_behalf"

    /** The agent's published lifecycle state, or null if absent/unknown. */
    fun agentState(attrs: Map<String, String>): AgentState? = attrs[AGENT_STATE_ATTRIBUTE]?.let(AgentState::fromWire)

    /** components-js's rule: an AGENT-kind participant that is not publishing on someone else's behalf. */
    fun isPrimaryAgent(isAgentKind: Boolean, attrs: Map<String, String>): Boolean =
        isAgentKind && PUBLISH_ON_BEHALF_ATTRIBUTE !in attrs

    /** A worker participant carrying media for `agentIdentity` (e.g. an avatar worker). */
    fun publishesForAgent(isAgentKind: Boolean, attrs: Map<String, String>, agentIdentity: String): Boolean =
        isAgentKind && attrs[PUBLISH_ON_BEHALF_ATTRIBUTE] == agentIdentity

    /**
     * LiveKit gives a frontend no interruption event, so barge-in is inferred:
     * the agent leaves `speaking` for `listening`/`thinking` while the local
     * user is an active speaker. Same known false positive as Web.
     */
    fun isInferredBargeIn(prev: AgentState, next: AgentState, userSpeaking: Boolean): Boolean =
        prev == AgentState.SPEAKING && (next == AgentState.LISTENING || next == AgentState.THINKING) && userSpeaking
}

/** What a participant is to the tracker; the glue attaches audio for AGENT and AGENT_WORKER. */
enum class LiveKitParticipantRole { AGENT, AGENT_WORKER, OTHER }

/**
 * The Web adapter's state machine without the SDK. The glue calls the event
 * methods on the main thread, `sink.onPcm` from the WebRTC thread, and `tick`
 * at 30 Hz on main. Until the agent publishes `lk.agent.state`, an energy
 * fallback on its audio drives the state (audible -> speaking, else
 * listening) -- older agents never publish one.
 */
class LiveKitAgentTracker {
    var agentIdentity: String? = null
        private set
    var state: AgentState = AgentState.IDLE
        private set
    var hasAudio = false
        private set
    private var agentStateSeen = false
    private var running = false

    /** Handed to the SDK's audio sink: the only part touched off the main thread. */
    val sink = LiveKitPcmSink()
    private val spectrum = SpectrumAnalyser()
    private val analysis = AudioAnalysis()

    var onState: ((AgentState) -> Unit)? = null
    var onInterrupt: (() -> Unit)? = null
    var onMetrics: ((VoiceMetrics) -> Unit)? = null

    /** Connected (or connecting): `initializing` until an agent shows up. */
    fun start() {
        running = true
        setState(AgentState.INITIALIZING)
    }

    fun stop() {
        running = false
        agentIdentity = null
        agentStateSeen = false
        audioDetached()
        setState(AgentState.IDLE)
    }

    fun role(identity: String, isAgentKind: Boolean, attrs: Map<String, String>): LiveKitParticipantRole {
        val agent = agentIdentity ?: return LiveKitParticipantRole.OTHER
        return when {
            identity == agent -> LiveKitParticipantRole.AGENT
            LiveKitAgent.publishesForAgent(isAgentKind, attrs, agent) -> LiveKitParticipantRole.AGENT_WORKER
            else -> LiveKitParticipantRole.OTHER
        }
    }

    /** A participant is present (already in the room, just joined, or its track was subscribed). Adopts the first primary agent. */
    fun participantSeen(identity: String, isAgentKind: Boolean, attrs: Map<String, String>): LiveKitParticipantRole {
        if (agentIdentity == null && LiveKitAgent.isPrimaryAgent(isAgentKind, attrs)) {
            agentIdentity = identity
            LiveKitAgent.agentState(attrs)?.let {
                agentStateSeen = true
                setState(it)
            }
        }
        return role(identity, isAgentKind, attrs)
    }

    /** `ParticipantAttributesChanged`: `changed` is the SDK's diff, `attrs` the participant's full set. */
    fun attributesChanged(
        identity: String,
        isAgentKind: Boolean,
        attrs: Map<String, String>,
        changed: Map<String, String>,
        localSpeaking: Boolean,
    ): LiveKitParticipantRole {
        // Attributes can land after the participant itself did.
        if (agentIdentity == null) return participantSeen(identity, isAgentKind, attrs)
        if (identity == agentIdentity && LiveKitAgent.AGENT_STATE_ATTRIBUTE in changed) {
            LiveKitAgent.agentState(attrs)?.let {
                agentStateSeen = true
                if (LiveKitAgent.isInferredBargeIn(state, it, localSpeaking)) onInterrupt?.invoke()
                setState(it)
            }
        }
        return role(identity, isAgentKind, attrs)
    }

    /** Returns true when the agent itself left: the glue drops its audio sink. */
    fun participantLeft(identity: String): Boolean {
        if (identity != agentIdentity) return false
        agentIdentity = null
        agentStateSeen = false
        audioDetached()
        if (running) setState(AgentState.INITIALIZING)
        return true
    }

    /** The glue attached its sink to an agent (or worker) audio track. */
    fun audioAttached() {
        sink.ring.drain()
        spectrum.reset()
        analysis.reset()
        hasAudio = true
    }

    fun audioDetached() {
        hasAudio = false
        sink.ring.drain()
    }

    /** 30 Hz on main: drain -> spectrum -> metrics; the energy fallback until a state is seen. */
    fun tick() {
        if (!hasAudio) return
        spectrum.push(sink.ring.drain())
        val m = analysis.read(spectrum.byteFrequencyData())
        onMetrics?.invoke(m)
        if (running && agentIdentity != null && !agentStateSeen) {
            setState(if (m.level > SPEAKING_LEVEL) AgentState.SPEAKING else AgentState.LISTENING)
        }
    }

    private fun setState(s: AgentState) {
        if (s == state) return
        state = s
        onState?.invoke(s)
    }

    companion object {
        /** Energy-fallback threshold: Web `LiveKitVoiceSource`'s 0.05, not the mic's 0.08. */
        const val SPEAKING_LEVEL = 0.05
    }
}

/**
 * The WebRTC-thread side of `LiveKitAgentTracker`: keeps channel 0 of the
 * sink's PCM in a locked ring (the `SampleRing` hand-off `LocalMicVoiceSource` uses).
 * Its `onPcm` takes exactly WebRTC `AudioTrackSink.onData`'s arguments.
 */
class LiveKitPcmSink {
    internal val ring = SampleRing(4096)
    private var scratch = FloatArray(0)

    /** Interleaved int16 PCM (what WebRTC delivers), channel 0 scaled `/32768`. Other bit depths are ignored. */
    fun onPcm(data: ByteBuffer, bitsPerSample: Int, channels: Int, frames: Int) {
        if (bitsPerSample != 16 || frames <= 0) return
        val stride = maxOf(1, channels)
        val shorts = data.duplicate().order(ByteOrder.nativeOrder()).asShortBuffer()
        val n = minOf(frames, shorts.remaining() / stride)
        if (scratch.size < n) scratch = FloatArray(n)
        for (i in 0 until n) scratch[i] = shorts.get(i * stride) / 32768f
        ring.write(scratch, n)
    }

    /** Normalized float samples (mono). */
    fun write(samples: FloatArray, n: Int = samples.size) = ring.write(samples, n)
}
