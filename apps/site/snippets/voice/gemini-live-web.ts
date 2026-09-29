import { GeminiLiveVoiceSource } from "@sinua/voice/gemini";

// Your endpoint mints an ephemeral token (`auth_tokens/…`) with the Gemini API key, which
// never reaches the browser (see Credentials). A reconnect resumes the session with a new
// token. The model, voice and instructions are locked into the token on the server.
export const voice = new GeminiLiveVoiceSource({ credentialUrl: "/api/voice/gemini" });
