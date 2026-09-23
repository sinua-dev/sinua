package snippets.perf

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.generated.SinuaOrb
import dev.sinua.view.generated.SinuaOrbPattern

@Composable
fun LabelledOrb() {
    Column {
        // An image with this name. Default: the spec's `name`, else the pattern.
        SinuaOrb(pattern = SinuaOrbPattern.LISTENING, contentDescription = "Assistant is listening", modifier = Modifier.size(160.dp))
        // Decorative (the text says it already): "" hides it from TalkBack.
        SinuaOrb(pattern = SinuaOrbPattern.LISTENING, contentDescription = "", modifier = Modifier.size(24.dp))
        BasicText("Listening…")
    }
}
