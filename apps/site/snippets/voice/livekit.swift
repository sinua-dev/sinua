import LiveKit
import SinuaLiveKit   // packages/ios-livekit

// Joins with your backend's access token and publishes the mic. The agent's state
// comes from its `lk.agent.state` attribute (LiveKit Agents set it).
func liveKitVoice(url: String, token: String) -> LiveKitVoiceSource {
    LiveKitVoiceSource(url: url, token: token)
}

// Already have a Room? Pass it; you keep ownership.
func liveKitVoice(room: Room) -> LiveKitVoiceSource {
    LiveKitVoiceSource(room: room)
}
