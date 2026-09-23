import { ElevenLabsVoiceSource } from "@sinua/voice/elevenlabs";

// A public agent: its agent_id. A private agent: a signed wss:// URL from your backend.
// The agent's user input format must be PCM (e.g. pcm_16000), set in the agent's settings.
export const publicAgent = new ElevenLabsVoiceSource({ credential: "agent_…" });

export async function privateAgent() {
  const { signedUrl } = await (await fetch("/api/elevenlabs-signed-url")).json();
  return new ElevenLabsVoiceSource({ credential: signedUrl });
}
