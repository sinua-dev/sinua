package snippets.voice

import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.generated.SinuaOrb
import dev.sinua.view.generated.SinuaOrbPattern
import dev.sinua.voice.SimulatedVoiceSource

@Composable
fun SimulatedConversation() {
    // No microphone, no network, no permission: for previews and demos.
    val voice = remember { SimulatedVoiceSource.sample("barge-in") }
    DisposableEffect(voice) {
        voice.connect()   // plays, and loops for the samples
        onDispose { voice.disconnect() }
    }
    SinuaOrb(pattern = SinuaOrbPattern.GLOWING, voice = voice, modifier = Modifier.size(160.dp))
}
