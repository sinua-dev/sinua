package snippets.voice

import dev.sinua.gemini.GeminiLiveVoiceSource

// `token`: an ephemeral `auth_tokens/…` your backend mints with the Gemini API key.
fun geminiVoice(token: String) = GeminiLiveVoiceSource(credential = token, instructions = "Keep answers short.")
