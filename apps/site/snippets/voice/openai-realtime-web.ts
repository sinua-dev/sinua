import { OpenAIRealtimeVoiceSource } from "@sinua/voice/openai";

// Your endpoint mints a short-lived `ek_…` with your API key, which never reaches the
// browser (see Credentials). The source POSTs to it on every connect and reconnect,
// since an `ek_` works once. The model, voice and instructions are set on the server.
export const voice = new OpenAIRealtimeVoiceSource({ credentialUrl: "/api/voice/openai" });
