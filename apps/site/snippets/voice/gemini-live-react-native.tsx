import { createVoiceSource } from "@sinua/react-native";

// Your backend mints the ephemeral `auth_tokens/…`; the API key never ships in the app.
export async function geminiVoice(backend: string) {
  const { token } = await (await fetch(`${backend}/gemini-live-token`, { method: "POST" })).json();
  return createVoiceSource({ vendor: "gemini", credential: token, instructions: "Keep answers short." });
}
