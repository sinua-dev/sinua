package snippets.voice

import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.unit.dp
import dev.sinua.view.SinuaAvatar
import dev.sinua.voice.VoiceSource

@Composable
fun Speaker(face: Painter, voice: VoiceSource) {
    // The image fills the middle (innerRadius, default 0.34 of the box); the ring goes round it.
    SinuaAvatar(face, label = "Ada", voice = voice, modifier = Modifier.size(56.dp))
}
