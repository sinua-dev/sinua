package snippets.values

import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.generated.SinuaSignal
import dev.sinua.view.generated.SinuaSignalPattern

@Composable
fun SignalKeys(quality: Double, accuracy: Double, blips: Int) {
    // How whole the sphere is, 0 to 1: as it falls, points let go and warm to amber.
    SinuaSignal(pattern = SinuaSignalPattern.COHESION, quality = quality, modifier = Modifier.size(96.dp))
    // How tight the cloud is, 0 to 1: 1 is on the core.
    SinuaSignal(pattern = SinuaSignalPattern.HALO, accuracy = accuracy, modifier = Modifier.size(96.dp))
    // Stages (2 to 8) and how far along: the clouds light up in turn.
    SinuaSignal(pattern = SinuaSignalPattern.CONSTELLATION, steps = 4, progress = 0.5, modifier = Modifier.size(96.dp))
    // Points found (0 to 12): each flashes as the beam reaches it.
    SinuaSignal(pattern = SinuaSignalPattern.SCAN, blips = blips, modifier = Modifier.size(96.dp))
}
