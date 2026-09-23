import SinuaGeminiLive

// `token`: an ephemeral `auth_tokens/…` your backend mints with the Gemini API key.
func geminiVoice(token: String) -> GeminiLiveVoiceSource {
    GeminiLiveVoiceSource(credential: token, instructions: "Keep answers short.")
}
