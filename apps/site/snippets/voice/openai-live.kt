package snippets.voice

import android.content.Context
import dev.sinua.openai.OpenAILiveVoiceSource

// GPT-Live has no client credential: your endpoint opens the session with your key and
// answers the app's SDP offer. The source POSTs `{ sdp }` to it on every connect, and
// again when a session expires.
fun openAILiveVoice(context: Context) =
    OpenAILiveVoiceSource(context, sessionUrl = "https://api.example.com/voice/openai-live")
