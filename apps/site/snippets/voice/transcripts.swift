import Foundation
import SinuaVoice
import SinuaOpenAI   // packages/ios-openai

@MainActor
final class Captions {
    private(set) var bubbles: [String: String] = [:]
    let voice = SharedVoiceSource.of(
        OpenAILiveVoiceSource(sessionURL: URL(string: "https://api.example.com/voice/openai-live")!)
    )
    private var stop: (() -> Void)?

    func start() async throws {
        // `text` is the whole turn so far (replace, don't append); `final` comes once per turn.
        stop = voice.listenTranscript { [weak self] u in
            self?.bubbles[u.turnId] = u.text
        }
        try await voice.connect()   // after subscribing, so the first turn isn't missed
    }
}
