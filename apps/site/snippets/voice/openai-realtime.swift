import Foundation
import SinuaOpenAI   // packages/ios-openai (WebRTC via LiveKit's build)

// Your endpoint mints a short-lived `ek_…` with your API key, which never ships in the app
// (see Credentials). The source POSTs to it on every connect and reconnect, since an `ek_`
// works once. The model, voice and instructions are set on the server.
func openAIVoice() -> OpenAIRealtimeVoiceSource {
    OpenAIRealtimeVoiceSource(credentialUrl: URL(string: "https://api.example.com/voice/openai")!)
}
