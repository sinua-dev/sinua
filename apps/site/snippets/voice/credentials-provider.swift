import Foundation
import SinuaOpenAI
import SinuaVoice

// When the request needs more than a POST (your app's auth header, say), pass a provider.
// It runs on every connect and reconnect.
func openAIVoice(appToken: @escaping @Sendable () async throws -> String) -> OpenAIRealtimeVoiceSource {
    OpenAIRealtimeVoiceSource(credential: .provider {
        var request = URLRequest(url: URL(string: "https://api.example.com/voice/openai")!)
        request.httpMethod = "POST"
        request.setValue("Bearer \(try await appToken())", forHTTPHeaderField: "Authorization")
        let (data, _) = try await URLSession.shared.data(for: request)
        return try CredentialSource.decode(vendor: "openai", data)
    })
}
