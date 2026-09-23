package dev.sinua.voice

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.nio.ByteBuffer
import java.nio.ByteOrder
import kotlin.math.PI
import kotlin.math.sin

/**
 * `LiveKitAgentTracker` -- the SDK-free core of :sinua-livekit -- driven by fake
 * participants and PCM; the same scenarios as the iOS LiveKitAgentTests and the
 * Web adapter's rules (packages/voice/src/livekitAgent.ts + LiveKitVoiceSource.ts).
 */
class LiveKitAgentTest {
    private val agentAttrs = mapOf("lk.agent.state" to "listening")

    private class Box {
        val states = mutableListOf<AgentState>()
        var interrupts = 0
        val metrics = mutableListOf<VoiceMetrics>()
    }

    private fun tracker(): Pair<LiveKitAgentTracker, Box> {
        val t = LiveKitAgentTracker()
        val box = Box()
        t.onState = { box.states += it }
        t.onInterrupt = { box.interrupts++ }
        t.onMetrics = { box.metrics += it }
        return t to box
    }

    private fun sine(n: Int, amp: Float, rate: Double = 48_000.0, freq: Double = 440.0) = FloatArray(n) { amp * sin(2 * PI * freq * it / rate).toFloat() }

    private fun state(s: String) = mapOf("lk.agent.state" to s)

    // --- rules

    @Test
    fun attributeMappingIsIdentityAndIgnoresUnknown() {
        for (s in AgentState.entries) assertEquals(s, LiveKitAgent.agentState(state(s.wire)))
        assertNull(LiveKitAgent.agentState(state("dancing")))
        assertNull(LiveKitAgent.agentState(emptyMap()))
    }

    @Test
    fun discoveryRules() {
        val worker = mapOf("lk.publish_on_behalf" to "a")
        assertTrue(LiveKitAgent.isPrimaryAgent(true, emptyMap()))
        assertFalse(LiveKitAgent.isPrimaryAgent(false, emptyMap()))
        assertFalse(LiveKitAgent.isPrimaryAgent(true, worker))
        assertTrue(LiveKitAgent.publishesForAgent(true, worker, "a"))
        assertFalse(LiveKitAgent.publishesForAgent(true, worker, "b"))
        assertFalse(LiveKitAgent.publishesForAgent(false, worker, "a"))
    }

    @Test
    fun bargeInTruthTable() {
        for (prev in AgentState.entries) {
            for (next in AgentState.entries) {
                for (speaking in listOf(false, true)) {
                    val want = prev == AgentState.SPEAKING &&
                        (next == AgentState.LISTENING || next == AgentState.THINKING) && speaking
                    assertEquals(
                        "$prev->$next speaking=$speaking",
                        want,
                        LiveKitAgent.isInferredBargeIn(prev, next, speaking),
                    )
                }
            }
        }
    }

    // --- tracker

    @Test
    fun adoptsPrimaryAgentNotWorkerOrStandard() {
        val (t, box) = tracker()
        t.start()
        val worker = mapOf("lk.publish_on_behalf" to "agent")
        assertEquals(LiveKitParticipantRole.OTHER, t.participantSeen("user", false, emptyMap()))
        assertEquals(LiveKitParticipantRole.OTHER, t.participantSeen("avatar", true, worker))
        assertNull(t.agentIdentity)
        assertEquals(LiveKitParticipantRole.AGENT, t.participantSeen("agent", true, agentAttrs))
        assertEquals(LiveKitParticipantRole.AGENT_WORKER, t.role("avatar", true, worker))
        assertEquals("first agent wins", LiveKitParticipantRole.OTHER, t.participantSeen("agent2", true, emptyMap()))
        assertEquals(listOf(AgentState.INITIALIZING, AgentState.LISTENING), box.states)
    }

    @Test
    fun attributesLandingAfterJoinSetTheState() {
        val (t, box) = tracker()
        t.start()
        t.participantSeen("agent", true, emptyMap())
        assertEquals(listOf(AgentState.INITIALIZING), box.states)
        t.attributesChanged("agent", true, state("thinking"), state("thinking"), false)
        assertEquals(listOf(AgentState.INITIALIZING, AgentState.THINKING), box.states)
    }

    @Test
    fun attributesAdoptWhenNoAgentYet() {
        val (t, box) = tracker()
        t.start()
        t.attributesChanged("agent", true, state("speaking"), state("speaking"), false)
        assertEquals("agent", t.agentIdentity)
        assertEquals(listOf(AgentState.INITIALIZING, AgentState.SPEAKING), box.states)
    }

    @Test
    fun unrelatedAndUnknownAttributeChangesAreIgnored() {
        val (t, box) = tracker()
        t.start()
        t.participantSeen("agent", true, agentAttrs)
        t.attributesChanged("agent", true, agentAttrs + ("x" to "1"), mapOf("x" to "1"), false)
        t.attributesChanged("agent", true, state("dancing"), state("dancing"), false)
        t.attributesChanged("user", false, state("speaking"), state("speaking"), false)
        assertEquals(listOf(AgentState.INITIALIZING, AgentState.LISTENING), box.states)
    }

    @Test
    fun bargeInFiresOnlyWhenUserIsSpeaking() {
        val (t, box) = tracker()
        t.start()
        t.participantSeen("agent", true, state("speaking"))
        t.attributesChanged("agent", true, agentAttrs, agentAttrs, false)
        assertEquals("normal end of turn", 0, box.interrupts)
        t.attributesChanged("agent", true, state("speaking"), state("speaking"), true)
        t.attributesChanged("agent", true, agentAttrs, agentAttrs, true)
        assertEquals(1, box.interrupts)
        assertEquals(
            listOf(
                AgentState.INITIALIZING,
                AgentState.SPEAKING,
                AgentState.LISTENING,
                AgentState.SPEAKING,
                AgentState.LISTENING,
            ),
            box.states,
        )
    }

    @Test
    fun agentLeavingReturnsToInitializingAndStopGoesIdle() {
        val (t, box) = tracker()
        t.start()
        t.participantSeen("agent", true, agentAttrs)
        t.audioAttached()
        assertFalse(t.participantLeft("user"))
        assertTrue(t.participantLeft("agent"))
        assertNull(t.agentIdentity)
        assertFalse(t.hasAudio)
        t.stop()
        assertEquals(
            listOf(AgentState.INITIALIZING, AgentState.LISTENING, AgentState.INITIALIZING, AgentState.IDLE),
            box.states,
        )
    }

    @Test
    fun energyFallbackUntilAStateIsPublished() {
        val (t, _) = tracker()
        t.start()
        t.participantSeen("agent", true, emptyMap())
        t.audioAttached()
        val loud = sine(1600, 0.5f)
        t.sink.write(loud)
        t.tick()
        assertEquals(AgentState.SPEAKING, t.state)
        t.sink.write(FloatArray(1600))
        repeat(30) { t.tick() } // release decays the smoothed level
        assertEquals(AgentState.LISTENING, t.state)
        t.attributesChanged("agent", true, state("thinking"), state("thinking"), false)
        t.sink.write(loud)
        t.tick()
        assertEquals(AgentState.THINKING, t.state)
    }

    @Test
    fun noMetricsWithoutAudio() {
        val (t, box) = tracker()
        t.start()
        t.participantSeen("agent", true, agentAttrs)
        t.tick()
        assertTrue(box.metrics.isEmpty())
    }

    /** The sink path is the mic path: same samples in, same metrics out as feeding `SpectrumAnalyser` directly. */
    @Test
    fun pcmSinkMatchesDirectAnalysis() {
        val (t, box) = tracker()
        t.start()
        t.participantSeen("agent", true, agentAttrs)
        t.audioAttached()
        val spectrum = SpectrumAnalyser()
        val analysis = AudioAnalysis()
        val want = mutableListOf<VoiceMetrics>()
        for (chunk in 0 until 10) {
            val s = sine(1600, chunk / 10f, freq = 300.0 + chunk * 100)
            t.sink.write(s)
            t.tick()
            spectrum.push(s)
            want += analysis.read(spectrum.byteFrequencyData())
        }
        assertEquals(want, box.metrics)
    }

    /** WebRTC `AudioTrackSink.onData`: interleaved native-order int16; only channel 0 counts. */
    @Test
    fun int16InterleavedKeepsChannelZero() {
        val (t, box) = tracker()
        t.start()
        t.participantSeen("agent", true, agentAttrs)
        t.audioAttached()
        val mono = sine(1600, 0.5f)
        val buf = ByteBuffer.allocateDirect(mono.size * 2 * 2).order(ByteOrder.nativeOrder())
        for (v in mono) {
            buf.putShort((v * 32768).toInt().toShort())
            buf.putShort(Short.MAX_VALUE) // channel 1: must be ignored
        }
        buf.flip()
        t.sink.onPcm(buf, bitsPerSample = 16, channels = 2, frames = mono.size)
        assertEquals("onPcm must not consume the caller's buffer", 0, buf.position())
        t.tick()
        val spectrum = SpectrumAnalyser()
        spectrum.push(FloatArray(mono.size) { (mono[it] * 32768).toInt().toShort() / 32768f })
        assertEquals(listOf(AudioAnalysis().read(spectrum.byteFrequencyData())), box.metrics)
    }

    @Test
    fun nonInt16PcmIsIgnored() {
        val (t, box) = tracker()
        t.start()
        t.participantSeen("agent", true, agentAttrs)
        t.audioAttached()
        t.sink.onPcm(ByteBuffer.allocateDirect(64), bitsPerSample = 8, channels = 1, frames = 64)
        t.tick()
        assertEquals(listOf(VoiceMetrics(0.0, List(MAX_AUDIO_BANDS) { 0.0 })), box.metrics)
    }
}
