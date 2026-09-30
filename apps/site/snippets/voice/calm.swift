import SwiftUI
import Sinua
import SinuaVoice

struct CalmSignal: View {
    @State private var voice = LocalMicVoiceSource()

    var body: some View {
        // The voice states add a glow and a pulse; your overrides go on top of them,
        // in every state. Here: a softer glow and no pulse.
        SinuaView(
            pattern: "waveform",
            overrides: ["glowStrength": 0.15, "pulseStrength": 0],
            voice: voice
        )
        .frame(width: 220, height: 120)
    }
}
