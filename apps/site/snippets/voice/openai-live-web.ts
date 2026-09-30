import { OpenAILiveVoiceSource } from "@sinua/voice/openai";

// GPT-Live has no browser credential: your endpoint opens the session with your key
// (see the server tab) and answers the source's SDP offer. The source POSTs `{ sdp }`
// to it on every connect, and again when a session expires.
export const voice = new OpenAILiveVoiceSource({ sessionUrl: "/api/voice/openai-live" });
