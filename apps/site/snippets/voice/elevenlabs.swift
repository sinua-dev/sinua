import Foundation
import SinuaElevenLabs

// The agent's user input format must be PCM (e.g. pcm_16000), set in the agent's settings.

// A private agent: your endpoint signs a `wss://` URL with your API key (see Credentials),
// a new one on every connect.
func privateAgent() -> ElevenLabsVoiceSource {
    ElevenLabsVoiceSource(credentialUrl: URL(string: "https://api.example.com/voice/elevenlabs")!)
}

// A public agent needs no backend: its agent id is the credential.
func publicAgent() -> ElevenLabsVoiceSource {
    ElevenLabsVoiceSource(credential: "agent_…")
}
