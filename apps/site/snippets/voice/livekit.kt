package snippets.voice

import android.content.Context
import dev.sinua.livekit.LiveKitVoiceSource
import io.livekit.android.room.Room

// Your endpoint signs a room token and answers `{ credential, url }` (see Credentials).
// The source joins the room and publishes the mic. The agent's state comes from its
// `lk.agent.state` attribute (LiveKit Agents set it).
fun liveKitVoice(context: Context) =
    LiveKitVoiceSource.withCredentialUrl(context, "https://api.example.com/voice/livekit")

// Already have a Room? Pass it; you keep ownership.
fun liveKitVoice(room: Room) = LiveKitVoiceSource(room)
