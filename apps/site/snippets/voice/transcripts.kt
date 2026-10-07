package snippets.voice

import android.content.Context
import dev.sinua.openai.OpenAILiveVoiceSource
import dev.sinua.voice.SharedVoiceSource

// Captions under the visual. `text` is the whole turn so far (replace, don't append);
// `final` comes once per turn. Sinua keeps nothing and sends the text nowhere.
class Captions(context: Context) {
    val bubbles = mutableMapOf<String, String>()
    val voice = SharedVoiceSource.of(
        OpenAILiveVoiceSource(context, sessionUrl = "https://api.example.com/voice/openai-live"),
    )
    private val stop = voice.listenTranscript { u -> bubbles[u.turnId] = u.text }

    fun start() = voice.connect() // after subscribing, so the first turn isn't missed

    fun close() = stop()
}
