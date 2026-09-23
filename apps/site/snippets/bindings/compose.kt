package snippets.bindings

import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.generated.SinuaRing

@Composable
fun DailyRings(specJson: String, steps: Double, waterMl: Double, activeMinutes: Double) {
    // activity-rings.fxspec.json's `bindings` map the inputs; pass only your raw numbers.
    // An input you leave out keeps its target at the design's value.
    SinuaRing(
        spec = specJson,
        inputs = mapOf("steps" to steps, "waterMl" to waterMl, "activeMinutes" to activeMinutes),
        modifier = Modifier.size(120.dp),
    )
}
