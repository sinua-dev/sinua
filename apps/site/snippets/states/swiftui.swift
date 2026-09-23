import SwiftUI
import Sinua
import SinuaVoice

struct AssistantOrb: View {
    let specJSON: String    // an .fxspec.json with a `states` map
    let voice: VoiceSource? // with a voice, the view follows the agent's state on its own
    var state: String?      // ... or drive it yourself: "listening", "thinking", "speaking"

    var body: some View {
        SinuaOrb(spec: specJSON, state: state, voice: voice)
            .frame(width: 160, height: 160)
    }
}
