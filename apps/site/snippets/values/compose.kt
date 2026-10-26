package snippets.values

import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.generated.SinuaSignal
import dev.sinua.view.generated.SinuaSignalPattern

@Composable
fun UploadDune(fraction: Double) {   // 0 to 1
    SinuaSignal(pattern = SinuaSignalPattern.DUNE, progress = fraction, modifier = Modifier.size(160.dp, 64.dp))
}
