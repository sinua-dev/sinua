import { createVoiceSource } from "@sinua/react-native";

// Your endpoint signs a room token and answers `{ credential, url }` (see Credentials).
// This vendor is opt-in per platform:
// iOS: add `pod "SinuaCore/LiveKit"` (with LiveKit's podspecs source).
// Android: sinua.voiceVendors=gemini,elevenlabs,livekit in gradle.properties.
export const voice = createVoiceSource({ vendor: "livekit", credentialUrl: "https://api.example.com/voice/livekit" });
