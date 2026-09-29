// npm i livekit-client (an optional peer: only apps using LiveKit install it)
import { Room } from "livekit-client";
import { LiveKitVoiceSource } from "@sinua/voice/livekit";

// Your endpoint signs a room token and answers `{ credential, url }` (see Credentials).
// The source joins the room and publishes the mic. The agent's state comes from its
// `lk.agent.state` attribute (LiveKit Agents set it).
export const voice = new LiveKitVoiceSource({ credentialUrl: "/api/voice/livekit" });

// Already have a Room (e.g. from @livekit/components-react)? Pass it; you keep ownership.
export const fromRoom = (room: Room) => new LiveKitVoiceSource({ room });
