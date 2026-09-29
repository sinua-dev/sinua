package dev.sinua.core

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import dev.sinua.voice.AgentState
import dev.sinua.voice.MainDispatcher
import dev.sinua.voice.SimulatedVoiceSource
import dev.sinua.voice.VoiceMetrics
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.core_engine.conversationAt
import uniffi.core_engine.conversationSample

/**
 * The simulated conversation on Android: the engine through UniFFI agrees with
 * spec/conversation-vectors.json (the file the Web and iOS tests read too), and
 * the source plays the script as a VoiceSource. No AudioRecord, no mic.
 */
@RunWith(AndroidJUnit4::class)
class SimulatedVoiceSourceTests {
    /** Runs posts inline and never schedules: time is driven by [SimulatedVoiceSource.advance]. */
    private object Manual : MainDispatcher {
        override fun post(r: Runnable) {}
        override fun postDelayed(r: Runnable, delayMs: Long) {}
        override fun remove(r: Runnable) {}
    }

    @Test
    fun theEngineMatchesTheSharedVectors() {
        val text = InstrumentationRegistry.getInstrumentation().context.assets
            .open("conversation-vectors.json").bufferedReader().use { it.readText() }
        val doc = JSONObject(text)
        val bands = doc.getInt("bands").toUInt()
        val cases = doc.getJSONArray("cases")
        assertTrue(cases.length() > 100)
        assertEquals(SimulatedVoiceSource.BUILT_IN_SAMPLES, SimulatedVoiceSource.sampleNames)
        for (i in 0 until cases.length()) {
            val c = cases.getJSONObject(i)
            val f = conversationAt(conversationSample(c.getString("sample"))!!, c.getDouble("t"), bands)
            assertEquals(c.getString("state"), f.state)
            assertEquals(c.getInt("turn"), f.turn.toInt())
            assertEquals(c.getDouble("level"), f.level, 1e-9)
            val want = c.getJSONArray("bands")
            f.bands.forEachIndexed { j, b -> assertEquals(want.getDouble(j), b, 1e-9) }
        }
    }

    @Test
    fun playsTheScriptWithOneBargeInFlash() {
        val src = SimulatedVoiceSource(conversationSample("barge-in")!!, bands = 8, main = Manual, autoTick = false)
        val states = mutableListOf<AgentState>()
        var interrupts = 0
        var last: VoiceMetrics? = null
        src.onStateChange { states += it }
        src.onInterrupt { interrupts++ }
        src.onMetrics { last = it }
        src.connect()
        repeat((src.duration * 30).toInt()) { src.advance(1.0 / 30) }
        assertEquals(
            listOf(AgentState.IDLE, AgentState.LISTENING, AgentState.THINKING, AgentState.SPEAKING, AgentState.LISTENING),
            states.take(5),
        )
        assertEquals(1, interrupts)
        assertEquals(8, last!!.bands.size)
        src.disconnect()
        assertEquals(AgentState.IDLE, states.last())
        assertEquals(0.0, last!!.level, 0.0)
    }

    @Test
    fun mutedSilencesTheUsersTurnsNotTheAgentsAndReportsTheConnection() {
        val src = SimulatedVoiceSource(conversationSample("calendar")!!, main = Manual, autoTick = false, loop = false)
        var last: VoiceMetrics? = null
        val conn = mutableListOf<Boolean>()
        src.onMetrics { last = it }
        src.onConnectionChange { conn += it }
        src.connect()
        val user = src.turns.first { it.voice == "user" }
        val agent = src.turns.first { it.voice == "agent" }
        src.seek(user.start + user.seconds / 2)
        assertTrue(last!!.level > 0)
        src.setMuted(true)
        assertEquals(0.0, last!!.level, 0.0)
        assertTrue(last!!.bands.all { it == 0.0 })
        src.seek(agent.start + agent.seconds / 2)
        assertTrue("the agent still talks", last!!.level > 0)
        src.disconnect()
        assertEquals(listOf(true, false), conn)
    }

    @Test
    fun aBadScriptThrowsWithItsPath() {
        val e = runCatching { SimulatedVoiceSource("""{"turns":[{"state":"idle","seconds":0}]}""", main = Manual) }
            .exceptionOrNull()
        assertTrue("$e", e?.message?.contains("/turns/0/seconds") == true)
        assertTrue(runCatching { SimulatedVoiceSource.sample("nope") }.isFailure)
    }
}
