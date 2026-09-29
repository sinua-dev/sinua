import SwiftUI
import Sinua

struct CoachView: View {
    var body: some View {
        SinuaView(
            pattern: "glowing",
            labels: ["listening": "Koç dinliyor", "speaking": "Koç konuşuyor"],   // your words, per state
            announce: true,   // state changes are spoken (VoiceOver), politely
            haptics: true     // a light tap when the agent starts listening
        )
        .accessibilityLabel("Coach")
    }
}
