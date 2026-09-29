package snippets.voice

import android.content.Context
import dev.sinua.openai.OpenAIRealtimeVoiceSource

// Your endpoint mints a short-lived `ek_…` with your API key, which never ships in the app
// (see Credentials). The source POSTs to it on every connect and reconnect, since an `ek_`
// works once. The model, voice and instructions are set on the server.
fun openAIVoice(context: Context) =
    OpenAIRealtimeVoiceSource.withCredentialUrl(context, "https://api.example.com/voice/openai")
