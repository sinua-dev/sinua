package snippets.voice

import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import dev.sinua.view.generated.SinuaEdge
import dev.sinua.view.generated.SinuaEdgePattern
import dev.sinua.voice.VoiceSource

@Composable
fun AssistantScreen(voice: VoiceSource) {
    Box(Modifier.fillMaxSize()) {
        BasicText("Your app")
        // A Box overlay above everything; it draws only, so taps reach what's beneath.
        SinuaEdge(
            pattern = SinuaEdgePattern.FRAMING,
            voice = voice,
            cornerRadius = 0.12,
            contentDescription = "", // decorative
            modifier = Modifier.fillMaxSize(),
        )
    }
}
