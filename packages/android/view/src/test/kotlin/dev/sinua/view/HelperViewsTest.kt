package dev.sinua.view

import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** SinuaVoiceMessage helpers (pure parts; JVM). */
class HelperViewsTest {
    @Test fun voiceMessageKeys() {
        val keys = sinuaVoiceMessageKeys(List(80) { 0.5 }, 1.4)
        assertEquals(1.0, keys["progress"]!!, 0.0)
        assertEquals(0.5, keys["envelope63"]!!, 0.0)
        assertNull("at most 64 values", keys["envelope64"])
    }
}
