import { createVoiceSource } from "@sinua/react-native";

// A private agent: your endpoint signs a `wss://` URL, a new one on every connect (see Credentials).
export const privateAgent = createVoiceSource({ vendor: "elevenlabs", credentialUrl: "https://api.example.com/voice/elevenlabs" });

// A public agent needs no backend: its agent id is the credential.
export const publicAgent = createVoiceSource({ vendor: "elevenlabs", credential: "agent_…" });
