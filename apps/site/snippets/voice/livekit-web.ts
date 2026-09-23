// npm i livekit-client (an optional peer: only apps using LiveKit install it)
import { Room } from "livekit-client";
import { LiveKitVoiceSource } from "@sinua/voice/livekit";

// The source joins with your backend's access token and publishes the mic. The agent's
// state comes from its `lk.agent.state` attribute (LiveKit Agents set it).
export async function liveKitVoice() {
  const { url, token } = await (await fetch("/api/livekit-token")).json();
  return new LiveKitVoiceSource({ url, token });
}

// Already have a Room (e.g. from @livekit/components-react)? Pass it; you keep ownership.
export const fromRoom = (room: Room) => new LiveKitVoiceSource({ room });
