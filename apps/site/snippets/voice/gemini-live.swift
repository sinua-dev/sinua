import Foundation
import SinuaGeminiLive

// Your endpoint mints an ephemeral `auth_tokens/…` with the Gemini API key (see Credentials).
// A reconnect resumes the session with a new token. The model, voice and instructions are
// locked into the token on the server.
func geminiVoice() -> GeminiLiveVoiceSource {
    GeminiLiveVoiceSource(credentialUrl: URL(string: "https://api.example.com/voice/gemini")!)
}
