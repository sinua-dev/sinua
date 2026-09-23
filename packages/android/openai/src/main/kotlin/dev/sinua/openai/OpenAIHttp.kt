package dev.sinua.openai

import dev.sinua.voice.OpenAIRealtimeSignaling
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.withContext
import okhttp3.MediaType.Companion.toMediaType
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.RequestBody.Companion.toRequestBody

/** Sends an `OpenAIRealtimeSignaling.Request` with OkHttp; returns (status, body text). */
class OpenAIHttp(private val client: OkHttpClient = OkHttpClient()) {
    suspend fun send(r: OpenAIRealtimeSignaling.Request): Pair<Int, String> = withContext(Dispatchers.IO) {
        val req = Request.Builder()
            .url(r.url)
            .apply { r.headers.forEach { (k, v) -> header(k, v) } }
            // Bytes, not String: String.toRequestBody would append "; charset=utf-8" to
            // the content type, and the Web adapter sends exactly `application/sdp`.
            .post(r.body.toByteArray().toRequestBody(r.contentType.toMediaType()))
            .build()
        client.newCall(req).execute().use { it.code to (it.body?.string() ?: "") }
    }
}
