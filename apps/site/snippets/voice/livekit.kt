package snippets.voice

import android.content.Context
import dev.sinua.livekit.LiveKitVoiceSource
import io.livekit.android.room.Room

// Joins with your backend's access token and publishes the mic. The agent's state
// comes from its `lk.agent.state` attribute (LiveKit Agents set it).
fun liveKitVoice(context: Context, url: String, token: String) = LiveKitVoiceSource.owned(context, url, token)

// Already have a Room? Pass it; you keep ownership.
fun liveKitVoice(room: Room) = LiveKitVoiceSource(room)
