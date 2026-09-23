package dev.sinua.websocket

import dev.sinua.voice.LiveSocket
import dev.sinua.voice.LiveSocketFactory
import okhttp3.OkHttpClient
import okhttp3.Request
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okio.ByteString
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicBoolean

/**
 * OkHttp's WebSocket for the native vendor VoiceSources (:sinua-gemini,
 * :sinua-elevenlabs share it, so an app gets one OkHttp). Unlike a browser
 * WebSocket it sets request headers, so credentials never go in the URL.
 * Subprotocols (ElevenLabs' `convai`) go in `Sec-WebSocket-Protocol`.
 * Callbacks arrive on OkHttp's threads.
 */
class OkHttpLiveSocketFactory(
    private val client: OkHttpClient = OkHttpClient.Builder()
        .readTimeout(0, TimeUnit.MILLISECONDS) // a live session idles between turns
        .pingInterval(20, TimeUnit.SECONDS)
        .build(),
) : LiveSocketFactory {
    override fun open(
        url: String,
        headers: Map<String, String>,
        protocols: List<String>,
        onText: (String) -> Unit,
        onClose: (Throwable?) -> Unit,
    ): LiveSocket {
        val req = Request.Builder().url(url).apply {
            headers.forEach { (k, v) -> header(k, v) }
            if (protocols.isNotEmpty()) header("Sec-WebSocket-Protocol", protocols.joinToString(", "))
        }.build()
        val closed = AtomicBoolean(false)
        val ws = client.newWebSocket(
            req,
            object : WebSocketListener() {
                override fun onMessage(webSocket: WebSocket, text: String) {
                    if (!closed.get()) onText(text)
                }

                // Gemini sends its JSON in binary frames too.
                override fun onMessage(webSocket: WebSocket, bytes: ByteString) {
                    if (!closed.get()) onText(bytes.utf8())
                }

                override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
                    webSocket.close(1000, null)
                    if (closed.compareAndSet(false, true)) onClose(ClosedException(code, reason))
                }

                override fun onFailure(webSocket: WebSocket, t: Throwable, response: Response?) {
                    if (closed.compareAndSet(false, true)) onClose(t)
                }
            },
        )
        return object : LiveSocket {
            override fun send(text: String) {
                ws.send(text)
            }

            override fun close() {
                if (closed.compareAndSet(false, true)) ws.close(1000, null)
            }
        }
    }

    class ClosedException(val code: Int, reason: String) :
        Exception("closed (code $code${if (reason.isEmpty()) "" else ": $reason"})")
}
