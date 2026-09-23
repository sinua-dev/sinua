import { OpenAIRealtimeVoiceSource } from "@sinua/voice/openai";

// Your backend mints a short-lived `ek_…` key (POST /v1/realtime/client_secrets with
// your API key, which never reaches the browser). An `ek_` is single-use, so give the
// source a function: it asks again on every reconnect.
export const voice = new OpenAIRealtimeVoiceSource({
  getCredential: async () => {
    const res = await fetch("/api/openai-realtime-key", { method: "POST" });
    return (await res.json()).key as string;
  },
  instructions: "You are a concise voice assistant.",
});
