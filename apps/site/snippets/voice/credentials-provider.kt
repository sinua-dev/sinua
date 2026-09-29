package snippets.voice

import android.content.Context
import dev.sinua.openai.OpenAIRealtimeVoiceSource
import dev.sinua.voice.CredentialSource
import java.net.HttpURLConnection
import java.net.URL

// When the request needs more than a POST (your app's auth header, say), pass a provider.
// It runs on a background thread on every connect and reconnect, so blocking I/O is fine.
fun openAIVoice(context: Context, appToken: () -> String) =
    OpenAIRealtimeVoiceSource(
        context,
        CredentialSource.provider {
            val conn = URL("https://api.example.com/voice/openai").openConnection() as HttpURLConnection
            conn.requestMethod = "POST"
            conn.setRequestProperty("Authorization", "Bearer ${appToken()}")
            CredentialSource.decode("openai", conn.inputStream.bufferedReader().use { it.readText() })
        },
    )
