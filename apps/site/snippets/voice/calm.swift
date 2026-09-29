import SwiftUI
import Sinua
import SinuaVoice

struct CalmSignal: View {
    @State private var voice = LocalMicVoiceSource()

    var body: some View {
        // The voice states add particles, glow and pulse; your overrides go on top of them,
        // in every state. Here: no particles, a softer glow, no pulse.
        SinuaView(
            pattern: "waveform",
            overrides: ["particleStrength": 0, "glowStrength": 0.15, "pulseStrength": 0],
            voice: voice
        )
        .frame(width: 220, height: 120)
    }
}
