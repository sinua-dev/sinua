package dev.sinua.voice

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertSame
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * The voice button's state machine against spec/voice-button-cases.json (the table Web and
 * iOS read too), the controller on a fake source, and the SharedVoiceSource fan-out.
 * Plain JVM: no Android, no audio.
 */
class VoiceButtonTest {
    private fun event(s: String): VoiceButtonEvent {
        val i = s.indexOf(':')
        val type = if (i < 0) s else s.substring(0, i)
        val arg = if (i < 0) "" else s.substring(i + 1)
        return when (type) {
            "press" -> VoiceButtonEvent.Press
            "release" -> VoiceButtonEvent.Release
            "end" -> VoiceButtonEvent.End
            "connectOk" -> VoiceButtonEvent.ConnectOk
            "connectFail" -> VoiceButtonEvent.ConnectFail(arg)
            "dropped" -> VoiceButtonEvent.Dropped
            "muted" -> VoiceButtonEvent.MuteChanged(arg == "true")
            else -> error("unknown event $s")
        }
    }

    @Test fun theStateMachineFollowsTheSharedTable() {
        val cases = JSONObject(File("../../spec/voice-button-cases.json").readText()).getJSONArray("cases")
        assertTrue(cases.length() >= 10)
        for (c in 0 until cases.length()) {
            val case = cases.getJSONObject(c)
            val name = case.getString("name")
            val mode = VoiceButtonMode.fromWire(case.getString("mode"))!!
            val canMute = case.getBoolean("canMute")
            var m = VoiceButtonModel()
            val steps = case.getJSONArray("steps")
            for (i in 0 until steps.length()) {
                val step = steps.getJSONArray(i)
                val ev = step.getString(0)
                val (next, fx) = voiceButtonStep(m, event(ev), mode, canMute)
                assertEquals("$name: after $ev", step.getString(1), next.state.wire)
                val want = step.getJSONArray(2).let { a -> (0 until a.length()).map { a.getString(it) } }
                assertEquals("$name: effects of $ev", want, fx.map { it.wire })
                m = next
            }
        }
    }

    /** One callback of each kind, like every real source. Reports connection from connect(). */
    private class Fake(val fail: Throwable? = null, val async: Boolean = false) : VoiceSource {
        var metricsCb: ((VoiceMetrics) -> Unit)? = null
        var stateCb: ((AgentState) -> Unit)? = null
        var connectionCb: ((Boolean) -> Unit)? = null
        var errorCb: ((Throwable) -> Unit)? = null
        val muted = mutableListOf<Boolean>()
        var disconnects = 0

        override fun connect() {
            fail?.let { throw it }
            if (!async) connectionCb?.invoke(true)
        }

        override fun disconnect() {
            disconnects++
            connectionCb?.invoke(false)
            stateCb?.invoke(AgentState.IDLE)
        }

        override fun onMetrics(cb: (VoiceMetrics) -> Unit) {
            metricsCb = cb
        }

        override fun onStateChange(cb: (AgentState) -> Unit) {
            stateCb = cb
        }

        override fun onConnectionChange(cb: (Boolean) -> Unit) {
            connectionCb = cb
        }

        override fun onError(cb: (Throwable) -> Unit) {
            errorCb = cb
        }

        override val reportsConnection: Boolean get() = true
        override val supportsMute: Boolean get() = true

        override fun setMuted(muted: Boolean) {
            this.muted.add(muted)
        }
    }

    @Test fun sharedSourceFansOutAndIsTheSameInstance() {
        val src = Fake()
        val a = SharedVoiceSource.of(src)
        assertSame(a, SharedVoiceSource.of(src))
        assertSame(a, SharedVoiceSource.of(a))
        val one = mutableListOf<AgentState>()
        val two = mutableListOf<AgentState>()
        val off = a.listenState { one.add(it) }
        a.listenState { two.add(it) }
        src.stateCb!!(AgentState.LISTENING)
        off()
        src.stateCb!!(AgentState.SPEAKING)
        assertEquals(listOf(AgentState.LISTENING), one)
        assertEquals(listOf(AgentState.LISTENING, AgentState.SPEAKING), two)
    }

    @Test fun trackedViewsKeepTheirOwnTrackerAndFollowTheMute() {
        val src = Fake()
        val shared = SharedVoiceSource.of(src)
        val a = shared.track()
        val b = shared.track()
        src.metricsCb!!(VoiceMetrics(0.5, listOf(0.4)))
        assertEquals(0.5, a.overrides.metrics.level, 0.0)
        shared.setMuted(true)
        assertEquals(listOf(true), src.muted)
        assertTrue(a.overrides.muted)
        b.release()
        src.metricsCb!!(VoiceMetrics(0.9, listOf(0.4)))
        assertEquals(0.5, b.overrides.metrics.level, 0.0)
        assertEquals(0.9, a.overrides.metrics.level, 0.0)
    }

    @Test fun controllerConnectsMutesAndFollowsARemoteDrop() {
        val src = Fake()
        val c = VoiceButtonController(src)
        val states = mutableListOf<VoiceButtonState>()
        c.onChange { states.add(it.state) }
        c.press()
        assertEquals(VoiceButtonState.LISTENING, c.state)
        c.press()
        assertEquals(VoiceButtonState.MUTED, c.state)
        assertEquals(listOf(false, true), src.muted)
        src.stateCb!!(AgentState.IDLE) // an agent idle while connected is not a drop
        assertEquals(VoiceButtonState.MUTED, c.state)
        src.connectionCb!!(false)
        assertEquals(VoiceButtonState.READY, c.state)
        assertEquals(
            listOf(VoiceButtonState.CONNECTING, VoiceButtonState.LISTENING, VoiceButtonState.MUTED, VoiceButtonState.READY),
            states,
        )
    }

    @Test fun controllerWaitsForTheSessionAndShowsALateFailure() {
        // Android vendors return from connect() while the socket is still opening.
        val src = Fake(async = true)
        val c = VoiceButtonController(src)
        c.press()
        assertEquals(VoiceButtonState.CONNECTING, c.state)
        src.errorCb!!(IllegalStateException("Gemini Live setup did not complete within 15s"))
        assertEquals(VoiceButtonState.ERROR, c.state)
        assertEquals("Gemini Live setup did not complete within 15s", c.reason)
        c.press()
        src.connectionCb!!(true)
        assertEquals(VoiceButtonState.LISTENING, c.state)
    }

    @Test fun controllerShowsAThrownConnect() {
        val c = VoiceButtonController(Fake(fail = SecurityException("RECORD_AUDIO not granted")))
        c.press()
        assertEquals(VoiceButtonState.ERROR, c.state)
        assertEquals("RECORD_AUDIO not granted", c.reason)
    }

    @Test fun aScreenReaderActivationInPushToTalkStartsLiveThenToggles() {
        val c = VoiceButtonController(Fake(), VoiceButtonMode.PUSH_TO_TALK)
        c.assistiveActivate()
        assertEquals(VoiceButtonState.LISTENING, c.state)
        c.assistiveActivate()
        assertEquals(VoiceButtonState.MUTED, c.state)
        c.assistiveActivate()
        assertEquals(VoiceButtonState.LISTENING, c.state)
        assertEquals(VoiceButtonMode.PUSH_TO_TALK, c.mode)
    }
}
