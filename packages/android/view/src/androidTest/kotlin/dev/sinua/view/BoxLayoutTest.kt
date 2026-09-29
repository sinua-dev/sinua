package dev.sinua.view

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clipToBounds
import androidx.compose.ui.test.junit4.createComposeRule
import androidx.compose.ui.unit.dp
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Rule
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.core_engine.patternLayout

/** Roadmap 9/10: which patterns fill the box, and the aspect a box-layout view passes. */
@RunWith(AndroidJUnit4::class)
class BoxLayoutTest {
    @get:Rule val compose = createComposeRule()

    /** Frames the pacer let through in 2 s of the test clock (16 ms frames). */
    private fun pacedIn2s(place: Modifier, maxFps: Double? = null): Int {
        var model: FxModel? = null
        compose.mainClock.autoAdvance = false
        compose.setContent {
            val m = remember { FxModel(FxInput.State("listening", 64u, emptyMap(), 1.0), null, null) }
            model = m
            Box(Modifier.size(100.dp).clipToBounds()) { FxCanvasForTest(m, maxFps, place) }
        }
        compose.mainClock.advanceTimeBy(200)
        val start = model!!.pacedFrames
        compose.mainClock.advanceTimeBy(2000)
        return model!!.pacedFrames - start
    }

    @Test fun aSmallViewRunsAt30Fps() {
        val n = pacedIn2s(Modifier.size(32.dp))
        assertTrue("32 dp: $n", n in 58..64)
    }

    @Test fun aFullSizeViewRunsAtDisplayRate() {
        val n = pacedIn2s(Modifier.size(80.dp))
        assertTrue("80 dp: $n", n >= 115)
    }

    @Test fun aViewClippedOutOfItsParentStops() {
        val n = pacedIn2s(Modifier.offset(y = 300.dp).size(80.dp))
        assertEquals("clipped out: no frames", 0, n)
    }

    @Test fun theEngineSaysWhichPatternsFillTheBox() {
        assertEquals("box", patternLayout("framing"))
        assertEquals("box", patternLayout("playing"))
        assertEquals("square", patternLayout("breathing"))
    }

    @Test fun aBoxLayoutModelDrawsAtTheBoxRatio() {
        val edge = FxModel(FxInput.State("framing", 64u, mapOf("idleOpacity" to 0.8), 1.0), null, null)
        assertTrue(edge.boxLayout)
        val wide = edge.frame(1_000_000L, running = false, reduced = true, aspect = 4.0)!!.frame
        val maxX = wide.polylines.flatMap { it.points }.maxOf { it.x }
        assertTrue("aspect 4: the rim runs to x ~ 256, got $maxX", maxX > 240 && maxX <= 256)
        val ring = FxModel(FxInput.State("completing", 64u, mapOf("progress" to 1.0), 1.0), null, null)
        assertFalse(ring.boxLayout)
    }
}
