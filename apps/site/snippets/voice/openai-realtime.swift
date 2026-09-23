import Foundation
import SinuaOpenAI   // packages/ios-openai (WebRTC via LiveKit's build)

// Your backend mints a short-lived `ek_…` key; the API key never ships in the app.
// An `ek_` is single-use, so the provider is asked again on every reconnect.
func openAIVoice(backend: URL) -> OpenAIRealtimeVoiceSource {
    OpenAIRealtimeVoiceSource(credentialProvider: {
        var request = URLRequest(url: backend.appendingPathComponent("openai-realtime-key"))
        request.httpMethod = "POST"
        let (data, _) = try await URLSession.shared.data(for: request)
        return String(decoding: data, as: UTF8.self)
    })
}
