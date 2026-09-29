package snippets.bindings

import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.generated.SinuaRing

@Composable
fun RulesRings(specJson: String, steps: Double, heartRate: Double) {
    // activity-rules.fxspec.json's `rules` pick the state from the inputs.
    SinuaRing(spec = specJson, inputs = mapOf("steps" to steps, "heartRate" to heartRate), modifier = Modifier.size(120.dp))
}
