import Foundation
import LiveKit
import SinuaLiveKit   // packages/ios-livekit

// Your endpoint signs a room token and answers `{ credential, url }` (see Credentials).
// The source joins the room and publishes the mic. The agent's state comes from its
// `lk.agent.state` attribute (LiveKit Agents set it).
func liveKitVoice() -> LiveKitVoiceSource {
    LiveKitVoiceSource(credentialUrl: URL(string: "https://api.example.com/voice/livekit")!)
}

// Already have a Room? Pass it; you keep ownership.
func liveKitVoice(room: Room) -> LiveKitVoiceSource {
    LiveKitVoiceSource(room: room)
}
