import SwiftUI
import Sinua
import SinuaVoice

struct TalkingOrb: View {
    // Any VoiceSource: the mic here, or a vendor source (see the vendor pages).
    @State private var voice = LocalMicVoiceSource()

    var body: some View {
        // The view binds the source; it never connects it.
        SinuaOrb(pattern: .speaking, voice: voice)
            .frame(width: 160, height: 160)
            .task {
                // Needs NSMicrophoneUsageDescription in Info.plist; the system asks once.
                do { try await voice.connect() } catch { print("voice failed to start:", error) }
            }
            .onDisappear { voice.disconnect() }
    }
}
