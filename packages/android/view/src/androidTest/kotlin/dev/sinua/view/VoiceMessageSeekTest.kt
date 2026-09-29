package dev.sinua.view

import androidx.compose.ui.unit.IntSize
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertEquals
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.core_engine.playbackSeekProgress

/** The voice message's seek mapping runs through the engine (needs the native library). */
@RunWith(AndroidJUnit4::class)
class VoiceMessageSeekTest {
    @Test fun seekMapsATouchOntoTheEnginesRow() {
        val box = IntSize(200, 40)
        assertEquals(0.0, sinuaVoiceMessageSeek(box, 0f), 0.0)
        assertEquals(1.0, sinuaVoiceMessageSeek(box, 200f), 0.0)
        assertEquals(0.5, sinuaVoiceMessageSeek(box, 100f), 1e-9)
        assertEquals(playbackSeekProgress(5.0, 2.5), sinuaVoiceMessageSeek(box, 100f), 0.0)
        assertEquals(0.0, sinuaVoiceMessageSeek(IntSize(0, 40), 10f), 0.0)
    }
}
