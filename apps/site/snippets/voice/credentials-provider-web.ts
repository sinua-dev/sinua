import { OpenAIRealtimeVoiceSource } from "@sinua/voice/openai";
import type { SinuaCredential } from "@sinua/voice";

// When the request needs more than a POST (a bearer token, a different host), pass a
// provider instead of a URL. It is called on every connect and reconnect.
export const voice = new OpenAIRealtimeVoiceSource({
  credential: async (): Promise<SinuaCredential> => {
    const res = await fetch("https://api.example.com/voice/openai", {
      method: "POST",
      headers: { Authorization: `Bearer ${await getAppToken()}` },
    });
    if (!res.ok) throw new Error(`credential endpoint: ${res.status}`);
    return res.json();
  },
});

declare function getAppToken(): Promise<string>;
