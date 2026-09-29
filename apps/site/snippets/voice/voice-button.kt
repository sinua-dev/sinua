package snippets.voice

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.SinuaVoiceButton
import dev.sinua.view.generated.SinuaOrb
import dev.sinua.view.generated.SinuaOrbPattern
import dev.sinua.voice.LocalMicVoiceSource
import dev.sinua.voice.VoiceButtonMode

@Composable
fun AssistantControls() {
    val source = remember { LocalMicVoiceSource() }
    Column {
        SinuaOrb(pattern = SinuaOrbPattern.GLOWING, voice = source, modifier = Modifier.size(160.dp))
        // Hold to talk: pressing unmutes, releasing mutes; the session stays up between presses.
        SinuaVoiceButton(source, mode = VoiceButtonMode.PUSH_TO_TALK)
    }
}
