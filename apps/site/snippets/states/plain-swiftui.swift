import SwiftUI
import Sinua
import SinuaVoice

// No spec file: one pattern, and the agent's state moves it.
struct PlainAssistantOrb: View {
    let voice: VoiceSource?       // with a voice, the view follows its state on its own
    var state: AgentState = .idle // ... or drive it yourself

    var body: some View {
        SinuaOrb(pattern: .working, state: state.rawValue, voice: voice)
            .frame(width: 160, height: 160)
    }
}
