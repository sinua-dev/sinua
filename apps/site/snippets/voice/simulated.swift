import SwiftUI
import Sinua

struct SimulatedConversation: View {
    // No microphone, no network, no permission: for previews and demos.
    @State private var voice = try? SimulatedVoiceSource(sample: "barge-in")

    var body: some View {
        SinuaOrb(pattern: .glowing, voice: voice)
            .frame(width: 160, height: 160)
            .task { try? await voice?.connect() }   // plays, and loops for the samples
            .onDisappear { voice?.disconnect() }
    }
}
