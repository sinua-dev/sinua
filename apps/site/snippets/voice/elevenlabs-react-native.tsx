import { createVoiceSource } from "@sinua/react-native";

// A public agent's id, or a signed wss:// URL from your backend for a private one.
export const agentVoice = (agentIdOrSignedUrl: string) =>
  createVoiceSource({ vendor: "elevenlabs", credential: agentIdOrSignedUrl });
