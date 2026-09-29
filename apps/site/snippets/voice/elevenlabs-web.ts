import { ElevenLabsVoiceSource } from "@sinua/voice/elevenlabs";

// The agent's user input format must be PCM (e.g. pcm_16000), set in the agent's settings.

// A private agent: your endpoint signs a `wss://` URL with your API key (see Credentials),
// a new one on every connect.
export const privateAgent = new ElevenLabsVoiceSource({ credentialUrl: "/api/voice/elevenlabs" });

// A public agent needs no backend: its agent id is the credential.
export const publicAgent = new ElevenLabsVoiceSource({ credential: "agent_…" });
