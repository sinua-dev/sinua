package dev.sinua.view

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith

/**
 * A view model plays a one-shot effect (docs/fx-view.md, *One-shot effects*): each new
 * trigger once, its keys for its duration, its words spoken at once. No audio.
 */
@RunWith(AndroidJUnit4::class)
class EffectPlaybackTest {
    @Test
    fun eachTriggerPlaysOnceForItsDurationAndIsSpoken() {
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            var now = 100.0
            val said = mutableListOf<String>()
            val m = FxModel(FxInput.State("completing", 64u, emptyMap(), 1.0), null, null)
            m.now = { now }
            m.speak = { said += it }
            m.appLabel = "Goal"
            m.a11yOptions(emptyMap(), null, haptics = false, rules = false)
            val success = SinuaEffectTrigger(SinuaEffect.SUCCESS)
            m.play(success)
            assertEquals(listOf("Done"), said)
            assertEquals(1.0, m.effectKeys(false)["effectCode"]!!, 0.0)
            now += 0.5
            assertEquals(0.5, m.effectKeys(true)["effectAge"]!!, 1e-9)
            assertEquals(1.0, m.effectKeys(true)["effectReduced"]!!, 0.0)
            m.play(success) // the same value: not replayed
            assertEquals(listOf("Done"), said)
            now += 0.5
            assertTrue("ended after 0.9 s", m.effectKeys(false).isEmpty())
            assertEquals(false, m.effectRunning)
            m.a11yOptions(mapOf("effect:celebrate" to "Hedef tamam"), null, haptics = false, rules = false)
            m.play(SinuaEffectTrigger(SinuaEffect.CELEBRATE))
            assertEquals(listOf("Done", "Hedef tamam"), said)
            m.a11yOptions(emptyMap(), false, haptics = false, rules = false)
            m.play(SinuaEffectTrigger(SinuaEffect.ERROR))
            assertEquals("announce = false keeps quiet", listOf("Done", "Hedef tamam"), said)
            assertEquals(2.0, m.effectKeys(false)["effectCode"]!!, 0.0)
        }
    }
}
