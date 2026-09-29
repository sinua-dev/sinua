import Sinua
import SwiftUI

// DevinFit's Soul, the shape of it: the Studio file ships in the app, the view draws
// it, and the chat's "AI is answering" becomes a state. (The app also writes the day's
// nutrition colour into the file before it is drawn.)
struct Soul: View {
    let spec: String // soul.fxspec.json
    let isAnswering: Bool

    var body: some View {
        SinuaView(spec: spec, state: isAnswering ? "thinking" : nil)
    }
}
