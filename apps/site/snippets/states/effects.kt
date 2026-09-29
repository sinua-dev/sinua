package snippets.states

import androidx.compose.foundation.layout.Column
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.clickable
import androidx.compose.foundation.text.BasicText
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.SinuaEffect
import dev.sinua.view.SinuaEffectTrigger
import dev.sinua.view.SinuaView

@Composable
fun Coach() {
    var effect by remember { mutableStateOf<SinuaEffectTrigger?>(null) }
    Column {
        SinuaView(pattern = "tracking", effect = effect, modifier = Modifier.size(160.dp))
        // Each new trigger plays once, on top of the current state.
        BasicText("Log workout", Modifier.clickable { effect = SinuaEffectTrigger(SinuaEffect.CELEBRATE) })
    }
}
