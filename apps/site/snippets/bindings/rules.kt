package snippets.bindings

import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.generated.SinuaSignal

@Composable
fun RulesActivity(specJson: String, steps: Double, heartRate: Double) {
    // activity-rules.fxspec.json's `rules` pick the state from the inputs.
    SinuaSignal(spec = specJson, inputs = mapOf("steps" to steps, "heartRate" to heartRate), modifier = Modifier.size(120.dp))
}
