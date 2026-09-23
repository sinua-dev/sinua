import { createVoiceSource } from "@sinua/react-native";

// `getCredential` is called again on every reconnect, which a single-use `ek_` needs.
// iOS: add `pod "SinuaCore/OpenAI"`. Android: sinua.voiceVendors=…,openai
export function openAIVoice(backend: string) {
  return createVoiceSource({
    vendor: "openai",
    getCredential: async () => (await (await fetch(`${backend}/openai-realtime-key`, { method: "POST" })).json()).key,
  });
}
