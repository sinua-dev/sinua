import { createVoiceSource } from "@sinua/react-native";

// Your endpoint mints a short-lived `ek_…` (see Credentials); it's asked again on every
// connect and reconnect, since an `ek_` works once.
// iOS: add `pod "SinuaCore/OpenAI"`. Android: sinua.voiceVendors=…,openai
export const voice = createVoiceSource({ vendor: "openai", credentialUrl: "https://api.example.com/voice/openai" });
