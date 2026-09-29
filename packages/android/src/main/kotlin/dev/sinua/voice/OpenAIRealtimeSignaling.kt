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

    /** The SDP offer -> `POST /v1/realtime/calls` with the ephemeral key; the answer SDP comes back as text. */
    fun callsRequest(sdpOffer: String, ephemeralKey: String, url: String = CALLS_URL) =
        Request(url, mapOf("Authorization" to "Bearer $ephemeralKey"), "application/sdp", sdpOffer)

    fun answer(status: Int, body: String): String {
        check(status, body)
        if (!body.startsWith("v=")) throw SignalingException.Malformed("the calls response isn't an SDP answer")
        return body
    }

    private fun check(status: Int, body: String) {
        if (status in 200..299) return
        if (RealtimeReconnect.isRetryable(status)) throw SignalingException.Retryable(status, body)
        throw SignalingException.Fatal(status, body)
    }
}
