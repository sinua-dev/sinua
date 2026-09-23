import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.SinuaView

@Composable
fun Visual() {
    SinuaView(pattern = "working", overrides = mapOf("glowRadius" to 3.0, "glowStrength" to 0.6), modifier = Modifier.size(160.dp))
}

// Or use the file: Spec menu → orb-working.fxspec.json (File tab).
