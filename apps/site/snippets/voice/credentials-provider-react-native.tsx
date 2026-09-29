import { createVoiceSource, type SinuaCredential } from "@sinua/react-native";

declare function getAppToken(): Promise<string>;

// When the request needs more than a POST (your app's auth header, say), pass a provider.
// It is called on every connect and reconnect.
export const voice = createVoiceSource({
  vendor: "openai",
  credential: async (): Promise<SinuaCredential> => {
    const res = await fetch("https://api.example.com/voice/openai", {
      method: "POST",
      headers: { Authorization: `Bearer ${await getAppToken()}` },
    });
    if (!res.ok) throw new Error(`credential endpoint: ${res.status}`);
    return res.json();
  },
});
