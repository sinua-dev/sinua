import SwiftUI
import Sinua

struct AssistantCard: View {
    let specJSON: String   // an .fxspec.json, e.g. exported from the Studio

    var body: some View {
        VStack(spacing: 24) {
            // Typed: one component per object, parameters as properties.
            SinuaSignal(pattern: .crystal, level: 0.65)
                .frame(width: 64, height: 64)

            // A spec file; `state` picks its lifecycle state.
            SinuaOrb(spec: specJSON, state: "listening")
                .frame(width: 160, height: 160)

            // The low-level view takes any pattern or spec.
            SinuaView(pattern: "speaking", theme: .dark)
                .frame(width: 96, height: 96)
        }
    }
}
