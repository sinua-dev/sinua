package snippets.bindings

import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.generated.SinuaSignal

@Composable
fun DailyActivity(specJson: String, steps: Double, activeMinutes: Double, heartRate: Double) {
    // activity-rings.fxspec.json's `bindings` map the inputs; pass only your raw numbers.
    // An input you leave out keeps its target at the design's value.
    SinuaSignal(
        spec = specJson,
        inputs = mapOf("steps" to steps, "activeMinutes" to activeMinutes, "heartRate" to heartRate),
        modifier = Modifier.size(120.dp),
    )
}
