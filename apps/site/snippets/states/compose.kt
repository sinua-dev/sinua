package snippets.states

import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.generated.SinuaOrb
import dev.sinua.voice.VoiceSource

@Composable
fun AssistantOrb(
    specJson: String,           // an .fxspec.json with a `states` map
    voice: VoiceSource?,        // with a voice, the view follows the agent's state on its own
    state: String? = null,      // ... or drive it yourself: "listening", "thinking", "speaking"
) {
    SinuaOrb(spec = specJson, state = state, voice = voice, modifier = Modifier.size(160.dp))
}
