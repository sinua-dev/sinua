package dev.sinua.view

import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.math.abs

/**
 * `PhaseClock`: the view clock is continuous across speed changes, so a lifecycle
 * state with its own speed (or an app changing `speed`) doesn't jump the pose.
 * The same cases as the Web mount tests and the iOS ClockTests.
 */
class ClockTest {
    private val frame = 1.0 / 60
    private fun advance(c: PhaseClock, frames: Int) = repeat(frames) { c.advance(frame) }

    @Test
    fun aConstantSpeedIsExactlyElapsedTimesSpeed() {
        val c = PhaseClock()
        advance(c, 30)
        assertEquals(c.elapsed * 1.3 * 1.5, c.phase(1.3, 1.5), 0.0)
    }

    @Test
    fun aSpeedChangeContinuesThePhaseInsteadOfJumping() {
        val c = PhaseClock()
        advance(c, 120) // two seconds in
        val before = c.phase(1.0, 1.0)
        c.advance(frame)
        val after = c.phase(1.0, 3.0)
        assertEquals("one step at the new speed, from the phase it had", before + frame * 3, after, 1e-12)
        assertTrue("not the rescaled elapsed", abs(after - c.elapsed * 3) > 1.0)
    }

    @Test
    fun speedZeroHoldsThePhaseAndResumesFromIt() {
        val c = PhaseClock()
        advance(c, 60)
        val held = c.phase(1.0, 1.0)
        repeat(60) {
            c.advance(frame)
            assertEquals("frozen while speed is 0", held, c.phase(1.0, 0.0), 0.0)
        }
        c.advance(frame)
        assertEquals("resumes from the held phase", held + frame, c.phase(1.0, 1.0), 1e-12)
    }

    @Test
    fun theSpecPathDivisionReturnsAnElapsedThatMultipliesBackToThePhase() {
        val c = PhaseClock()
        advance(c, 45)
        val phase = c.phase(2.0, 0.5)
        val at = phase / (2.0 * 0.5) // what the view hands FxSpecPlayer
        assertEquals(phase, at * 2.0 * 0.5, 1e-12)
    }
}
