package dev.sinua.view

import androidx.compose.ui.unit.dp
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Test

/** SinuaAvatar and SinuaVoiceMessage helpers (pure parts; JVM). */
class HelperViewsTest {
    @Test fun avatarImageSpansTwiceTheInnerRadius() {
        assertEquals(0.34, SINUA_AVATAR_INNER_RADIUS, 0.0)
        assertEquals(68f, sinuaAvatarImageDiameter(100.dp, 0.34).value, 1e-4f)
        assertEquals(44.8f, sinuaAvatarImageDiameter(56.dp, 0.4).value, 1e-4f)
    }

    @Test fun voiceMessageKeys() {
        val keys = sinuaVoiceMessageKeys(List(80) { 0.5 }, 1.4)
        assertEquals(1.0, keys["progress"]!!, 0.0)
        assertEquals(0.5, keys["envelope63"]!!, 0.0)
        assertNull("at most 64 values", keys["envelope64"])
    }
}
