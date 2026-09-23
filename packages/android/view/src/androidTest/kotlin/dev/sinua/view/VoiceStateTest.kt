package dev.sinua.view

import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.core_engine.voiceStateProfile

/**
 * Voice-state profiles as the view applies them for plain input (`pattern` + `state`):
 * the same cases as the Web mount tests and the iOS VoiceStateProfileTests. Instrumented
 * because the profile comes from the engine (.so). No audio: the states are passed directly.
 */
@RunWith(AndroidJUnit4::class)
class VoiceStateTest {
    private fun model(state: String?, overrides: Map<String, Double> = emptyMap()): FxModel {
        val m = FxModel(FxInput.State("working", 64u, overrides, 1.0), null, null)
        m.specState = state
        return m
    }

    private fun dots(m: FxModel): List<Triple<Double, Double, Double>> {
        m.frame(0L, running = true, reduced = false) // seeds the clock
        val frames = m.frame(16_666_667L, running = true, reduced = false)
        return frames!!.frame.dots.map { Triple(it.x, it.y, it.r) }
    }

    @Test
    fun theProfileExistsForTheVoiceStatesOnly() {
        val listening = voiceStateProfile("working", "listening")
        assertTrue("listening draws inward", (listening?.overrides?.get("audioStrength") ?: 0.0) < 0.0)
        assertEquals("micLevel", listening?.audioInput)
        assertNull("an app's own state name", voiceStateProfile("working", "goalReached"))
    }

    @Test
    fun aVoiceStateChangesTheFrameAndAnAppStateDoesNot() {
        val plain = dots(model(null))
        assertNotEquals("the voice state moves the pattern", plain, dots(model("listening")))
        assertEquals("an app's own state name is left alone", plain, dots(model("goalReached")))
    }

    @Test
    fun theAppsOwnOverridesWinOverTheProfile() {
        val profile = voiceStateProfile("working", "listening")!!
        val key = "glowStrength"
        val mine = (profile.overrides[key] ?: 0.0) + 0.3
        assertNotEquals(dots(model("listening")), dots(model("listening", mapOf(key to mine))))
    }
}
