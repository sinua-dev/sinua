import SwiftUI
import Sinua

struct Coach: View {
    @State private var effect: SinuaEffectTrigger?

    var body: some View {
        VStack {
            SinuaView(pattern: "tracking", effect: effect)
                .frame(width: 160, height: 160)
            // Each new trigger plays once, on top of the current state.
            Button("Log workout") { effect = SinuaEffectTrigger(.celebrate) }
        }
    }
}
