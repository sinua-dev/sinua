// OpenAI Realtime WebRTC signaling without WebRTC or an HTTP client: the two
// requests (as plain data) and their response handling, mirrored from packages/ios
// SinuaVoice/OpenAIRealtimeSignaling.swift. The :sinua-openai glue sends them
// with OkHttp; tests check the shape and the status mapping.
package dev.sinua.voice

import org.json.JSONObject

object OpenAIRealtimeSignaling {
    const val CALLS_URL = "https://api.openai.com/v1/realtime/calls"
    const val DEFAULT_MODEL = "gpt-realtime"
    const val DEFAULT_VOICE = "marin"

    data class Request(val url: String, val headers: Map<String, String>, val contentType: String, val body: String)

    sealed class SignalingException(message: String) : Exception(message) {
        /** 400/401/403 and other non-retryable statuses: don't retry. */
        class Fatal(val status: Int, val body: String) : SignalingException("OpenAI returned $status: $body")
        class Retryable(val status: Int, val body: String) : SignalingException("OpenAI returned $status: $body")
        class Malformed(message: String) : SignalingException(message)
    }

    /** WARP's pre-negotiated event channel id (any free id works; the same one goes in `dcid`). */
    const val WARP_DATA_CHANNEL_ID = 1

    /**
     * WARP's libwebrtc field trials (developers.openai.com `guides/realtime-webrtc-warp`):
     * DTLS 1.3, SNAP and SPED. Process-wide, and only read before the first peer connection factory.
     */
    const val WARP_FIELD_TRIALS =
        "WebRTC-ForceDtls13/Enabled/WebRTC-Sctp-Snap/Enabled/WebRTC-IceHandshakeDtls/Enabled/"

    /**
     * The SDP offer -> `POST /v1/realtime/calls` with the ephemeral key; the answer SDP comes back
     * as text. [dcid]: WARP's pre-negotiated event channel id, sent along as `?dcid=`.
     */
    fun callsRequest(sdpOffer: String, ephemeralKey: String, url: String = CALLS_URL, dcid: Int? = null) =
        Request(withDcid(url, dcid), mapOf("Authorization" to "Bearer $ephemeralKey"), "application/sdp", sdpOffer)

    private fun withDcid(url: String, dcid: Int?): String {
        if (dcid == null) return url
        val (base, fragment) = url.split('#', limit = 2).let { it[0] to it.getOrNull(1) }
        val kept = base.substringAfter('?', "").split('&').filter { it.isNotEmpty() && !it.startsWith("dcid=") }
        val query = (kept + "dcid=$dcid").joinToString("&")
        return base.substringBefore('?') + "?" + query + (fragment?.let { "#$it" } ?: "")
    }

    fun answer(status: Int, body: String): String {
        check(status, body)
        if (!body.startsWith("v=")) throw SignalingException.Malformed("the calls response isn't an SDP answer")
        return body
    }

    internal fun check(status: Int, body: String) {
        if (status in 200..299) return
        if (RealtimeReconnect.isRetryable(status)) throw SignalingException.Retryable(status, body)
        throw SignalingException.Fatal(status, body)
    }
}
