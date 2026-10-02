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

    @Test
    fun theTapHopIsSilentCarriesTheTapAndNeverCutsAnotherEffect() {
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            var now = 100.0
            val said = mutableListOf<String>()
            val m = FxModel(FxInput.State("completing", 64u, emptyMap(), 1.0), null, null)
            m.now = { now }
            m.speak = { said += it }
            m.appLabel = "Goal"
            m.a11yOptions(emptyMap(), null, haptics = false, rules = false)
            m.hop(0.5 to -0.25)
            assertTrue("the hop is silent", said.isEmpty())
            assertEquals(4.0, m.effectKeys(false)["effectCode"]!!, 0.0)
            assertEquals(0.5, m.effectKeys(false)["tapX"]!!, 0.0)
            assertEquals(-0.25, m.effectKeys(false)["tapY"]!!, 0.0)
            now += 0.2
            m.hop(1.0 to 1.0) // within 0.5 s of the last one: ignored
            assertEquals(0.5, m.effectKeys(false)["tapX"]!!, 0.0)
            now += 0.5
            assertTrue("ended after 0.6 s", m.effectKeys(false).isEmpty())
            m.play(SinuaEffectTrigger(SinuaEffect.SUCCESS))
            m.hop(0.0 to 0.0)
            assertEquals("a tap doesn't cut success", 1.0, m.effectKeys(false)["effectCode"]!!, 0.0)
            now += 1.0
            m.play(SinuaEffectTrigger(SinuaEffect.HOP))
            assertEquals(4.0, m.effectKeys(false)["effectCode"]!!, 0.0)
            assertTrue("a triggered hop has no tap", m.effectKeys(false)["tapX"] == null)
        }
    }

    @Test
    fun theExpressionEasesInOverHalfASecondAndNullAddsNothing() {
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            var now = 100.0
            val m = FxModel(FxInput.State("bean", 64u, emptyMap(), 1.0), null, null)
            m.now = { now }
            assertTrue("unset: the spec decides", m.expressionKeys(false).isEmpty())
            m.expression = "happy"
            m.expressionKeys(false)
            now += 0.3
            assertEquals(0.5, m.expressionKeys(false)["expressionHappy"]!!, 1e-9)
            now += 0.3
            assertEquals(1.0, m.expressionKeys(false)["expressionHappy"]!!, 0.0)
            m.expression = "sad"
            assertEquals("reduced motion cuts", 1.0, m.expressionKeys(true)["expressionSad"]!!, 0.0)
        }
    }

    @Test
    fun thePaletteResolvesThroughTheEngine() {
        InstrumentationRegistry.getInstrumentation().runOnMainSync {
            val m = FxModel(FxInput.State("bean", 64u, emptyMap(), 1.0), null, null)
            assertTrue("empty: the character's own", m.paletteKeys().isEmpty())
            m.palette = mapOf("bean" to "#2B1A12")
            assertEquals(1.0, m.paletteKeys()["palette.bean.w"]!!, 0.0)
            assertTrue("the tones follow", m.paletteKeys().containsKey("palette.beanDark.l"))
            assertTrue("a dark ground lifts the ink", m.paletteKeys()["palette.ink.l"]!! > 0.8)
            m.palette = mapOf("beam" to "#000000")
            assertTrue("an unknown slot adds nothing", m.paletteKeys().isEmpty())
        }
    }
}
