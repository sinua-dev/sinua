import { OpenAIRealtimeVoiceSource } from "@sinua/voice/openai";

// Your own backend opens the Realtime call with your key, so it can join it on OpenAI's
// sideband connection for tools and transcripts. The source sends its SDP offer there
// instead of to OpenAI, with your own short-lived token (any shape, never `sk-…`).
export const voice = new OpenAIRealtimeVoiceSource({
  callsUrl: "/api/voice/openai-call",
  credentialUrl: "/api/voice/openai-call-token",
});
