package snippets.realapp

import androidx.compose.runtime.Composable
import dev.sinua.view.SinuaView

// DevinFit's Soul, the shape of it: the Studio file ships in the app, the view draws
// it, and the chat's "AI is answering" becomes a state. (The app also writes the day's
// nutrition colour into the file before it is drawn.)
@Composable
fun Soul(spec: String /* soul.fxspec.json */, isAnswering: Boolean) {
    SinuaView(spec = spec, state = if (isAnswering) "thinking" else null)
}
