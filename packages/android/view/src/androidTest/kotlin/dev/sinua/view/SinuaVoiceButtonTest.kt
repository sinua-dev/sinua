package dev.sinua.view

import androidx.compose.ui.semantics.SemanticsActions
import androidx.compose.ui.semantics.SemanticsProperties
import androidx.compose.ui.semantics.getOrNull
import androidx.compose.ui.test.SemanticsMatcher
import androidx.compose.ui.test.assert
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.test.onNodeWithContentDescription
import androidx.compose.ui.test.performClick
import androidx.compose.ui.test.performSemanticsAction
import androidx.test.ext.junit.runners.AndroidJUnit4
import dev.sinua.voice.AgentState
import dev.sinua.voice.SharedVoiceSource
import dev.sinua.voice.VoiceMetrics
import dev.sinua.voice.VoiceSource
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith

/**
 * `SinuaVoiceButton`'s accessibility and wiring: a button whose name follows the state, a
 * TalkBack activation that works in push-to-talk, the "End voice session" action, and a view
 * next to it that keeps its own subscription and shows the mute. A fake source: no audio.
 */
@RunWith(AndroidJUnit4::class)
class SinuaVoiceButtonTest {
    @get:Rule val rule = createComposeRule()

    private class Fake : VoiceSource {
        var metricsCb: ((VoiceMetrics) -> Unit)? = null
        var connectionCb: ((Boolean) -> Unit)? = null
        val muted = mutableListOf<Boolean>()
        var disconnects = 0
        override fun connect() {
            connectionCb?.invoke(true)
        }
        override fun disconnect() {
            disconnects++
            connectionCb?.invoke(false)
        }
        override fun onMetrics(cb: (VoiceMetrics) -> Unit) {
            metricsCb = cb
        }
        override fun onStateChange(cb: (AgentState) -> Unit) {}
        override fun onConnectionChange(cb: (Boolean) -> Unit) {
            connectionCb = cb
        }
        override val reportsConnection: Boolean get() = true
        override val supportsMute: Boolean get() = true
        override fun setMuted(muted: Boolean) {
            this.muted += muted
        }
    }

    private fun hasClickLabel(label: String) = SemanticsMatcher("click label $label") {
        it.config.getOrNull(SemanticsActions.OnClick)?.label == label
    }

    @Test
    fun theNameFollowsTheStateAndAssistiveActivationToggles() {
        val src = Fake()
        rule.setContent { SinuaVoiceButton(src) }
        rule.onNodeWithContentDescription("Start voice").assert(hasClickLabel("Connects the voice session"))
        rule.onNodeWithContentDescription("Start voice").performSemanticsAction(SemanticsActions.OnClick)
        rule.onNodeWithContentDescription("Microphone on").assert(hasClickLabel("Mutes the microphone"))
        rule.onNodeWithContentDescription("Microphone on").performSemanticsAction(SemanticsActions.OnClick)
        rule.onNodeWithContentDescription("Microphone muted")
            .assert(SemanticsMatcher.expectValue(SemanticsProperties.Selected, true))
        assertEquals(listOf(false, true), src.muted)
        // The ✕ ends the session.
        rule.onNodeWithContentDescription("End voice session").performClick()
        rule.onNodeWithContentDescription("Start voice").assertExists()
        assertEquals(1, src.disconnects)
    }

    @Test
    fun pushToTalkUnderTalkBackStartsLiveThenToggles() {
        val src = Fake()
        rule.setContent { SinuaVoiceButton(src, mode = dev.sinua.voice.VoiceButtonMode.PUSH_TO_TALK) }
        rule.onNodeWithContentDescription("Start voice").performSemanticsAction(SemanticsActions.OnClick)
        rule.onNodeWithContentDescription("Microphone on").assert(hasClickLabel("Release to mute"))
        rule.onNodeWithContentDescription("Microphone on").performSemanticsAction(SemanticsActions.OnClick)
        rule.onNodeWithContentDescription("Microphone muted").assert(hasClickLabel("Hold to talk"))
    }

    @Test
    fun aViewOnTheSameSourceKeepsItsSubscriptionAndShowsTheMute() {
        val src = Fake()
        rule.setContent {
            SinuaView(pattern = "glowing", voice = src)
            SinuaVoiceButton(src)
        }
        rule.onNodeWithContentDescription("Start voice").performSemanticsAction(SemanticsActions.OnClick)
        rule.onNodeWithContentDescription("Microphone on").performSemanticsAction(SemanticsActions.OnClick)
        rule.waitForIdle()
        val shared = SharedVoiceSource.of(src)
        assertTrue(shared.muted)
        // Both the view and the button's ring are listening through the one fan-out.
        val seen = mutableListOf<Double>()
        val off = shared.listenMetrics { seen += it.level }
        src.metricsCb!!(VoiceMetrics(0.4, listOf(0.1)))
        off()
        assertEquals(listOf(0.4), seen)
    }
}
