package snippets.voice

import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.generated.SinuaCharacter
import dev.sinua.view.generated.SinuaCharacterPattern
import dev.sinua.voice.VoiceSource

@Composable
fun Assistant(voice: VoiceSource) {
    // It looks at you while you talk, looks away while it thinks, and its mouth
    // follows the agent's voice. `hue` turns the body; the eyes keep their colour.
    SinuaCharacter(
        pattern = SinuaCharacterPattern.BUZZY,
        hue = 190.0,
        voice = voice,
        contentDescription = "Buzzy",
        labels = mapOf("listening" to "Buzzy is listening", "speaking" to "Buzzy is speaking"),
        expression = "happy", // a mood your app picks
        palette = mapOf("accent" to "#FFFFFF"), // repaint a slot outright
        modifier = Modifier.size(160.dp),
    )
}
