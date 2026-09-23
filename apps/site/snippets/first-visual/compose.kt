package snippets.firstvisual

import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.generated.SinuaOrb
import dev.sinua.view.generated.SinuaOrbPattern

@Composable
fun BreathingOrb() {
    SinuaOrb(pattern = SinuaOrbPattern.BREATHING, modifier = Modifier.size(160.dp))
}
