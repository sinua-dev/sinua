package snippets.platforms

import androidx.compose.foundation.layout.Arrangement
import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.FxTheme
import dev.sinua.view.SinuaView
import dev.sinua.view.generated.SinuaOrb
import dev.sinua.view.generated.SinuaSignal
import dev.sinua.view.generated.SinuaSignalPattern

@Composable
fun AssistantCard(specJson: String) {   // an .fxspec.json, e.g. exported from the Studio
    Column(verticalArrangement = Arrangement.spacedBy(24.dp)) {
        // Typed: one component per object, parameters as arguments.
        SinuaSignal(pattern = SinuaSignalPattern.CRYSTAL, level = 0.65, modifier = Modifier.size(64.dp))
        // A spec file; `state` picks its lifecycle state.
        SinuaOrb(spec = specJson, state = "listening", modifier = Modifier.size(160.dp))
        // The low-level view takes any pattern or spec.
        SinuaView(pattern = "speaking", theme = FxTheme.DARK, modifier = Modifier.size(96.dp))
    }
}
