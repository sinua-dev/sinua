package dev.sinua.view.generated

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** The generated typed components (scripts/codegen): props -> engine overrides, and the spec check. */
class TypedComponentsTest {
    @Test
    fun ringPropsMapToEngineKeys() {
        assertEquals(mapOf("progress" to 0.4, "strokeWidth" to 0.1), SinuaRingProps(SinuaRingPattern.COMPLETING, progress = SinuaNumbers.of(0.4), strokeWidth = 0.1).toOverrides())
        assertEquals(
            mapOf("progress0" to 0.2, "progress1" to 0.5, "ringCount" to 2.0),
            SinuaRingProps(SinuaRingPattern.TRACKING, progress = SinuaNumbers.of(0.2, 0.5), ringCount = 2).toOverrides(),
        )
        assertEquals(mapOf("progress0" to 0.7), SinuaRingProps(SinuaRingPattern.TRACKING, progress = SinuaNumbers.of(0.7)).toOverrides())
        assertEquals(mapOf("progress" to 0.3), SinuaRingProps(SinuaRingPattern.COMPLETING, progress = SinuaNumbers.of(listOf(0.3, 0.9))).toOverrides())
        assertEquals(mapOf("fill" to 1.0, "marker" to 0.0), SinuaRingProps(SinuaRingPattern.MEASURING, fill = true, marker = false).toOverrides())
        assertEquals(mapOf("segment0" to 1.0, "segment1" to 0.5), SinuaRingProps(SinuaRingPattern.STEPPING, segment = listOf(1.0, 0.5)).toOverrides())
        assertEquals(
            mapOf("glowStrength" to 0.6, "glowBlend" to 1.0),
            SinuaRingProps(SinuaRingPattern.LOADING, glow = SinuaGlow(strength = 0.6, blend = SinuaGlow.Blend.ADDITIVE)).toOverrides(),
        )
    }

    @Test
    fun orbFlatParticlesAndTheMaterialStayApart() {
        val o = SinuaOrbProps(SinuaOrbPattern.WORKING, orbitParticles = 4, particles = SinuaParticles(count = 12, style = SinuaParticles.Style.ORBIT)).toOverrides()
        assertEquals(mapOf("particles" to 4.0, "particleCount" to 12.0, "particleStyle" to 2.0), o)
        assertEquals(emptyMap<String, Double>(), SinuaSignalProps(SinuaSignalPattern.WAVEFORM).toOverrides())
    }

    @Test
    fun specForAnotherObjectIsAnError() {
        val orb = """{"fxSpec":"1.8","object":"orb","pattern":"idle","states":{}}"""
        assertEquals(SinuaSpecError("ring", "orb"), sinuaSpecError(orb, "ring"))
        assertNull(sinuaSpecError(orb, "orb"))
        assertNull(sinuaSpecError("not json", "ring"))
    }
}
