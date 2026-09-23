package snippets.states

import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.generated.SinuaOrb
import dev.sinua.view.generated.SinuaOrbPattern
import dev.sinua.voice.AgentState
import dev.sinua.voice.VoiceSource

// No spec file: one pattern, and the agent's state moves it.
@Composable
fun PlainAssistantOrb(
    voice: VoiceSource?,                      // with a voice, the view follows its state on its own
    state: AgentState = AgentState.IDLE,      // ... or drive it yourself
) {
    SinuaOrb(
        pattern = SinuaOrbPattern.WORKING,
        state = state.wire,
        voice = voice,
        modifier = Modifier.size(160.dp),
    )
}
