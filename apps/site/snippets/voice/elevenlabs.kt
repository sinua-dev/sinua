package snippets.voice

import android.content.Context
import dev.sinua.elevenlabs.ElevenLabsVoiceSource
import dev.sinua.voice.CredentialSource

// The agent's user input format must be PCM (e.g. pcm_16000), set in the agent's settings.

// A private agent: your endpoint signs a `wss://` URL with your API key (see Credentials),
// a new one on every connect.
fun privateAgent(context: Context) =
    ElevenLabsVoiceSource(context, CredentialSource.url("https://api.example.com/voice/elevenlabs"))

// A public agent needs no backend: its agent id is the credential.
fun publicAgent(context: Context) = ElevenLabsVoiceSource(context, credential = "agent_…")
