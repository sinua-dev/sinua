// Transport + threading seams for the native vendor VoiceSources (Gemini Live
// and ElevenLabs). Dependency-free: the OkHttp implementation is the
// :sinua-websocket module, so this library never pulls a network stack.
package dev.sinua.voice

import android.os.Handler
import android.os.Looper

/** The one thing the vendor sources need from a WebSocket. `send` is safe from any thread. */
interface LiveSocket {
    fun send(text: String)
    fun close()
}

/**
 * Opens sockets. Callbacks may arrive on any thread; the sources hop to main
 * themselves. `protocols` are WebSocket subprotocols (ElevenLabs' `convai`).
 * The OkHttp implementation is the :sinua-websocket module.
 */
interface LiveSocketFactory {
    fun open(
        url: String,
        headers: Map<String, String>,
        protocols: List<String>,
        onText: (String) -> Unit,
        onClose: (Throwable?) -> Unit,
    ): LiveSocket
}

/** "The main thread", injectable so the sources run under plain JVM tests (no Looper there). */
interface MainDispatcher {
    fun post(r: Runnable)
    fun postDelayed(r: Runnable, delayMs: Long)
    fun remove(r: Runnable)
}

class LooperMainDispatcher : MainDispatcher {
    private val handler = Handler(Looper.getMainLooper())
    override fun post(r: Runnable) {
        handler.post(r)
    }
    override fun postDelayed(r: Runnable, delayMs: Long) {
        handler.postDelayed(r, delayMs)
    }
    override fun remove(r: Runnable) = handler.removeCallbacks(r)
}
