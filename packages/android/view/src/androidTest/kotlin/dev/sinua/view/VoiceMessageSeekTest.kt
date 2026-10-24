package dev.sinua.view

import androidx.compose.ui.unit.IntSize
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.core_engine.frameWithOverrides
import uniffi.core_engine.playbackSeekProgress

/** The voice message's seek mapping runs through the engine (needs the native library). */
@RunWith(AndroidJUnit4::class)
class VoiceMessageSeekTest {
    @Test fun seekInvertsTheDunesProjection() {
        val box = IntSize(200, 40)
        // Outside the dune it clamps; an empty box is 0.
        assertEquals(0.0, sinuaVoiceMessageSeek(box, 0f), 0.0)
        assertEquals(1.0, sinuaVoiceMessageSeek(box, 200f), 0.0)
        assertEquals(0.0, sinuaVoiceMessageSeek(IntSize(0, 40), 10f), 0.0)
        // The view passes the engine the box's ratio and the touch over the height.
        assertEquals(playbackSeekProgress(5.0, 2.5), sinuaVoiceMessageSeek(box, 100f), 0.0)
        // Monotonic 0..1 across the strip.
        var last = -1.0
        for (x in 0..200) {
            val p = sinuaVoiceMessageSeek(box, x.toFloat())
            assertTrue("x $x: $p", p in 0.0..1.0 && p >= last)
            last = p
        }
    }

    @Test fun aTouchOnThePlayheadReturnsTheProgress() {
        val box = IntSize(200, 40)
        val frame = frameWithOverrides("dune", 64u, 1.0, mapOf("aspect" to 5.0, "progress" to 0.3))!!
        // The playhead is the plane of white light; its mean x, in box heights, then in px.
        val head = frame.dots.filter { it.white > 0.85 }.map { it.x }
        assertTrue("the playhead is drawn", head.isNotEmpty())
        val x = head.average() / 64.0 * box.height
        assertEquals(0.3, sinuaVoiceMessageSeek(box, x.toFloat()), 0.02)
    }
}
