package snippets.voice

import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.SinuaView
import dev.sinua.voice.LocalMicVoiceSource

@Composable
fun CalmSignal() {
    val voice = remember { LocalMicVoiceSource() }
    // The voice states add a glow and a pulse; your overrides go on top of them,
    // in every state. Here: a softer glow and no pulse.
    SinuaView(
        pattern = "waveform",
        overrides = mapOf("glowStrength" to 0.15, "pulseStrength" to 0.0),
        voice = voice,
        modifier = Modifier.size(220.dp, 120.dp),
    )
}
