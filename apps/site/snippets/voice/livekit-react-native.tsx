import { createVoiceSource } from "@sinua/react-native";

// The access token comes from your backend. This vendor is opt-in per platform:
// iOS: add `pod "SinuaCore/LiveKit"` (with LiveKit's podspecs source).
// Android: sinua.voiceVendors=gemini,elevenlabs,livekit in gradle.properties.
export async function liveKitVoice(backend: string) {
  const { url, token } = await (await fetch(`${backend}/livekit-token`)).json();
  return createVoiceSource({ vendor: "livekit", url, token });
}
