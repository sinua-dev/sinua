import { GeminiLiveVoiceSource } from "@sinua/voice/gemini";

// Your backend mints an ephemeral token (`auth_tokens/…`) with the Gemini API key,
// which never reaches the browser.
export async function geminiVoice() {
  const { token } = await (await fetch("/api/gemini-live-token", { method: "POST" })).json();
  return new GeminiLiveVoiceSource({ credential: token, instructions: "Keep answers short." });
}
