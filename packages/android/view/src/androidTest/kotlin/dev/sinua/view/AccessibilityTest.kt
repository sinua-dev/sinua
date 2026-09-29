package dev.sinua.view

import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import dev.sinua.voice.AgentState
import dev.sinua.voice.VoiceMetrics
import dev.sinua.voice.VoiceSource
import org.junit.Assert.assertEquals
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * State-aware accessibility on Android (docs/fx-view.md, *Accessibility*): the name follows
 * the state, a held change is spoken once through the view's hook, the opt-in tap, and the
 * 1.9 rules glue. A fake source: no audio.
 */
@RunWith(AndroidJUnit4::class)
class AccessibilityTest {
    @get:Rule val rule = createComposeRule()

    private class Fake : VoiceSource {
        var stateCb: ((AgentState) -> Unit)? = null
        override fun connect() {}
        override fun disconnect() {}
        override fun onMetrics(cb: (VoiceMetrics) -> Unit) {}
        override fun onStateChange(cb: (AgentState) -> Unit) {
            stateCb = cb
        }
    }

    private val instr = InstrumentationRegistry.getInstrumentation()

    private fun onMain(block: () -> Unit) = instr.runOnMainSync(block)

    @Test
    fun theNameFollowsTheVoiceAndAHeldChangeIsSpokenOnce() {
        val src = Fake()
        lateinit var m: FxModel
        val said = mutableListOf<String>()
        var taps = 0
        onMain {
            m = FxModel(FxInput.State("glowing", 64u, emptyMap(), 1.0), src, null)
            m.speak = { said += it }
            m.tap = { taps++ }
            m.a11yOptions(emptyMap(), null, haptics = true, rules = false)
            m.refreshA11y()
        }
        onMain { assertEquals("glowing", m.a11yLabel) }
        onMain { src.stateCb!!(AgentState.LISTENING) }
        instr.waitForIdleSync()
        onMain { assertEquals("glowing, listening", m.a11yLabel) }
        assertEquals(1, taps)
        assertEquals(emptyList<String>(), said)
        Thread.sleep(1200)
        instr.waitForIdleSync()
        assertEquals(listOf("glowing, listening"), said)
        onMain { src.stateCb!!(AgentState.SPEAKING) }
        Thread.sleep(300)
        onMain { src.stateCb!!(AgentState.LISTENING) }
        Thread.sleep(1300)
        instr.waitForIdleSync()
        assertEquals(listOf("glowing, listening"), said)
        onMain { m.release() }
    }

    @Test
    fun rulesDeriveTheStateWithHysteresis() {
        val spec =
            """{"fxSpec":"1.9","object":"orb","pattern":"glowing","name":"Heart","states":{"intense":{}},""" +
                """"rules":[{"when":{"input":"hr","gt":150},"state":"intense","hysteresis":5}],""" +
                """"accessibility":{"states":{"intense":"Heart rate high"}}}"""
        val labels = mutableListOf<String>()
        onMain {
            val m = FxModel(FxInput.Spec(spec), null, null)
            m.a11yOptions(emptyMap(), false, haptics = false, rules = true)
            for (hr in listOf(140.0, 151.0, 148.0, 145.0)) {
                m.inputs = mapOf("hr" to hr)
                m.refreshA11y()
                labels += m.a11yLabel
            }
            m.a11yOptions(emptyMap(), false, haptics = false, rules = false)
            m.inputs = mapOf("hr" to 160.0)
            m.refreshA11y()
            labels += m.a11yLabel
        }
        assertEquals(listOf("Heart", "Heart rate high", "Heart rate high", "Heart", "Heart"), labels)
    }

    @Test
    fun theComposeSemanticsCarryTheStateAndLabelsWin() {
        val src = Fake()
        rule.setContent {
            SinuaView(pattern = "glowing", voice = src, labels = mapOf("speaking" to "Koç konuşuyor"))
        }
        rule.onNodeWithContentDescription("glowing").assertExists()
        rule.runOnIdle { src.stateCb!!(AgentState.SPEAKING) }
        rule.waitForIdle()
        rule.onNodeWithContentDescription("Koç konuşuyor")
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.Role, androidx.compose.ui.semantics.Role.Image))
    }
}
