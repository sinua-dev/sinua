import { createVoiceSource } from "@sinua/react-native";

// Your endpoint mints the ephemeral `auth_tokens/…` and locks the model, voice and
// instructions into it (see Credentials). A reconnect asks for a new one.
export const voice = createVoiceSource({ vendor: "gemini", credentialUrl: "https://api.example.com/voice/gemini" });
