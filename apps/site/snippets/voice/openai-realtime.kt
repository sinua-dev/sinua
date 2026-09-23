package snippets.voice

import android.content.Context
import dev.sinua.openai.OpenAIRealtimeVoiceSource

// Your backend mints a short-lived `ek_…` key; the API key never ships in the app.
// An `ek_` is single-use, so the provider is called again on every reconnect.
fun openAIVoice(context: Context, fetchKey: suspend () -> String) =
    OpenAIRealtimeVoiceSource(context, credentialProvider = fetchKey)
