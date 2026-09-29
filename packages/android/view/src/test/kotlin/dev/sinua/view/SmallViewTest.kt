package dev.sinua.view

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** Roadmap 10: a small view defaults to 30 fps; the app's maxFps or the spec's cap wins (pure; JVM). */
class SmallViewTest {
    @Test fun smallViewCap() {
        assertEquals(30.0, fxOptionMaxFps(null, null, small = true)!!, 0.0)
        assertNull(fxOptionMaxFps(null, null, small = false))
        assertEquals(60.0, fxOptionMaxFps(60.0, null, small = true)!!, 0.0)
        assertEquals("maxFps 0 is the app asking for display rate", 0.0, fxOptionMaxFps(0.0, null, small = true)!!, 0.0)
        assertNull("the spec's performance.maxFps decides", fxOptionMaxFps(null, 60.0, small = true))
        // Through the policy: 30 for a bare small view, the spec's 60 when it sets one.
        assertEquals(30.0, fxPerformance(false, fxOptionMaxFps(null, null, true)).maxFps!!, 0.0)
        assertEquals(60.0, fxPerformance(false, fxOptionMaxFps(null, 60.0, true), 60.0).maxFps!!, 0.0)
        assertNull(fxPerformance(false, fxOptionMaxFps(0.0, null, true)).maxFps)
    }
}
