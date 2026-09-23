package dev.sinua.view

import org.junit.Assert.assertEquals
import org.junit.Test

/** The shared pacer and low-power policy (pure; JVM). Same cases as the Web and Swift tests. */
class PerformanceTest {
    private fun drawsPerSecond(maxFps: Double?, hz: Double, seconds: Double = 10.0, jitter: Double = 0.0): Double {
        val p = FramePacer(maxFps)
        var seed = 1L
        var draws = 0
        for (i in 0 until (hz * seconds).toInt()) {
            seed = (seed * 16807) % 2147483647
            val r = seed.toDouble() / 2147483647 - 0.5
            if (p.shouldDraw(1000 + i * 1000 / hz + r * 2 * jitter)) draws++
        }
        return draws / seconds
    }

    @Test fun pacerIsExactWithoutDrift() {
        assertEquals(30.0, drawsPerSecond(30.0, 60.0), 0.1)
        assertEquals(30.0, drawsPerSecond(30.0, 120.0), 0.1)
        assertEquals(24.0, drawsPerSecond(24.0, 60.0), 0.2)
        assertEquals(30.0, drawsPerSecond(30.0, 60.0, jitter = 0.8), 0.3)
        assertEquals(60.0, drawsPerSecond(null, 60.0), 0.0)
        assertEquals(60.0, drawsPerSecond(0.0, 60.0), 0.0)
        assertEquals(60.0, drawsPerSecond(120.0, 60.0), 0.0)
    }

    @Test fun performancePolicy() {
        assertEquals(FxPerformance(null, emptyMap()), fxPerformance(false, null))
        assertEquals(
            FxPerformance(30.0, mapOf("glowStrength" to 0.0, "particleStrength" to 0.0)),
            fxPerformance(true, null),
        )
        assertEquals(FxPerformance(20.0, emptyMap()), fxPerformance(true, null, 20.0, true))
        assertEquals(FxPerformance(24.0, emptyMap()), fxPerformance(false, 24.0, 60.0))
        assertEquals(
            FxPerformance(24.0, mapOf("glowStrength" to 0.0, "particleStrength" to 0.0)),
            fxPerformance(true, null, 24.0),
        )
    }
}

class MaterialsPaintTest {
    /** Skia: sigma = 0.57735 * radius + 0.5, so the mask radius inverts it. */
    @org.junit.Test fun blurMaskRadiusInvertsSkiaConversion() {
        for (sigma in listOf(1.0, 2.0, 4.0, 13.44)) {
            val r = blurMaskRadius(sigma)
            org.junit.Assert.assertEquals(sigma, 0.57735 * r + 0.5, 1e-4)
        }
        org.junit.Assert.assertEquals(0.1f, blurMaskRadius(0.3), 0f) // floor for tiny sigma
    }

    /** Off-device (SDK_INT 0) behaves like API < 29: blur is not drawn -- the documented fallback path. */
    @org.junit.Test fun blurFallsBackBelowApi29() {
        org.junit.Assert.assertFalse(fxBlurSupported)
    }
}
