package snippets.perf

import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.FxLowPower
import dev.sinua.view.generated.SinuaOrb
import dev.sinua.view.generated.SinuaOrbPattern

@Composable
fun LowPowerOrb() {
    // AUTO (default) follows Battery Saver; ON / OFF force it.
    // Low power = the spec's `performance.lowPower` block, else 30 fps with glow and particles off.
    SinuaOrb(pattern = SinuaOrbPattern.SPEAKING, lowPower = FxLowPower.AUTO, modifier = Modifier.size(160.dp))
}
