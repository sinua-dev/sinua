import SwiftUI
import Sinua
import SinuaVoice

struct AssistantScreen: View {
    let voice: VoiceSource

    var body: some View {
        ZStack {
            Text("Your app")
            // Over everything, edge to edge, never in the way of a tap.
            SinuaEdge(pattern: .framing, cornerRadius: 0.12, voice: voice)
                .ignoresSafeArea()
                .allowsHitTesting(false)
                .accessibilityHidden(true)   // decorative
        }
    }
}
