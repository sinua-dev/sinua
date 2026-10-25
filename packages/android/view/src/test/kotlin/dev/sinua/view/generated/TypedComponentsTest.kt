package dev.sinua.view.generated

import androidx.compose.runtime.Composable
import dev.sinua.view.SinuaEffect
import dev.sinua.view.SinuaEffectTrigger
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** The generated typed components (scripts/codegen): props -> engine overrides, and the spec check. */
class TypedComponentsTest {
    @Test
    fun orbFlatParticlesAndTheMaterialStayApart() {
        val o = SinuaOrbProps(SinuaOrbPattern.WORKING, orbitParticles = 4, noise = SinuaNoise(strength = 0.5)).toOverrides()
        assertEquals(mapOf("particles" to 4.0, "noiseStrength" to 0.5), o)
        assertEquals(emptyMap<String, Double>(), SinuaSignalProps(SinuaSignalPattern.VEIL).toOverrides())
        assertEquals(emptyMap<String, Double>(), SinuaSignalProps(SinuaSignalPattern.DUNE).toOverrides())
    }

    @Test
    fun specForAnotherObjectIsAnError() {
        val orb = """{"fxSpec":"1.8","object":"orb","pattern":"idle","states":{}}"""
        assertEquals(SinuaSpecError("signal", "orb"), sinuaSpecError(orb, "signal"))
        assertNull(sinuaSpecError(orb, "orb"))
        assertNull(sinuaSpecError("not json", "signal"))
    }
}

/**
 * Compile-level: the typed components take SinuaView's accessibility and effect parameters
 * (both overloads). Never called; a JVM test can't compose.
 */
@Suppress("unused")
@Composable
private fun typedComponentsTakeAccessibilityAndEffects(trigger: SinuaEffectTrigger = SinuaEffectTrigger(SinuaEffect.SUCCESS)) {
    val labels = mapOf("listening" to "Coach is listening")
    SinuaOrb(SinuaOrbPattern.LISTENING, labels = labels, announce = false, haptics = true, effect = trigger)
}
