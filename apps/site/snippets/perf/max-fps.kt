package snippets.perf

import android.util.Log
import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.generated.SinuaOrb
import dev.sinua.view.generated.SinuaOrbPattern

@Composable
fun CalmOrb() {
    // Cap the frame rate; the clock keeps wall time, so the motion's speed is unchanged.
    SinuaOrb(
        pattern = SinuaOrbPattern.BREATHING,
        maxFps = 24.0,
        onFrame = { s -> if (s.computeMs + s.paintMs > 8) Log.d("Orb", "slow frame $s") },
        modifier = Modifier.size(160.dp),
    )
}
