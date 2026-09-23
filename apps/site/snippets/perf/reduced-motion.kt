package snippets.perf

import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.FxReducedMotion
import dev.sinua.view.generated.SinuaOrb
import dev.sinua.view.generated.SinuaOrbPattern

@Composable
fun QuietOrb() {
    // AUTO (default) follows "Remove animations": a still pose.
    // With a voice attached it still redraws (up to 30 Hz): the voice cue is information.
    SinuaOrb(pattern = SinuaOrbPattern.WORKING, reducedMotion = FxReducedMotion.AUTO, modifier = Modifier.size(160.dp))
}
