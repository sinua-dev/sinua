import Foundation
import SinuaOpenAI   // packages/ios-openai

// GPT-Live has no client credential: your endpoint opens the session with your key and
// answers the app's SDP offer. The source POSTs `{ sdp }` to it on every connect, and
// again when a session expires.
func openAILiveVoice() -> OpenAILiveVoiceSource {
    OpenAILiveVoiceSource(sessionURL: URL(string: "https://api.example.com/voice/openai-live")!)
}
