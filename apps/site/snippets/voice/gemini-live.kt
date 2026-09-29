package snippets.voice

import dev.sinua.gemini.GeminiLiveVoiceSource
import dev.sinua.voice.CredentialSource

// Your endpoint mints an ephemeral `auth_tokens/…` with the Gemini API key (see Credentials).
// A reconnect resumes the session with a new token. The model, voice and instructions are
// locked into the token on the server.
fun geminiVoice() = GeminiLiveVoiceSource(CredentialSource.url("https://api.example.com/voice/gemini"))
