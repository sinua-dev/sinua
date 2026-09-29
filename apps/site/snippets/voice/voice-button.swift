import SwiftUI
import Sinua
import SinuaVoice

struct AssistantControls: View {
    @State private var source = LocalMicVoiceSource()

    var body: some View {
        VStack {
            SinuaOrb(pattern: .glowing, voice: source).frame(width: 160, height: 160)
            // Hold to talk: pressing unmutes, releasing mutes; the session stays up between presses.
            SinuaVoiceButton(source: source, mode: .pushToTalk)
        }
    }
}
