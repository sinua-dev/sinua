package snippets.values

import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.generated.SinuaNumbers
import dev.sinua.view.generated.SinuaRing
import dev.sinua.view.generated.SinuaRingPattern

@Composable
fun UploadRing(fraction: Double) {   // 0 to 1
    SinuaRing(pattern = SinuaRingPattern.COMPLETING, progress = SinuaNumbers.of(fraction), modifier = Modifier.size(64.dp))
}

@Composable
fun ActivityRings(move: Double, exercise: Double, stand: Double) {
    // One value per ring, outermost first; `ringCount` sets how many are drawn.
    SinuaRing(
        pattern = SinuaRingPattern.TRACKING,
        progress = SinuaNumbers.of(listOf(move, exercise, stand)),
        ringCount = 3,
        modifier = Modifier.size(96.dp),
    )
}
