// OpenAI GPT-Live WebRTC signaling without WebRTC or an HTTP client, mirrored from
// packages/ios SinuaVoice/OpenAILiveSignaling.swift. GPT-Live has no client credential:
// your server opens the session (`POST /v1/live/sessions` with its project key), so the
// offer goes to *your* endpoint as `{ "sdp": … }` JSON, and the answer is OpenAI's 201
// JSON passed through (`{ session: { id }, transport: { sdp } }`) or the bare SDP text.
package dev.sinua.voice

import org.json.JSONObject

object OpenAILiveSignaling {
    data class Answer(val sdp: String, val sessionId: String?)

    /** The offer -> your session endpoint, with your own token when you have one. */
    fun sessionRequest(sdpOffer: String, token: String?, url: String) = OpenAIRealtimeSignaling.Request(
        url,
        if (token != null) mapOf("Authorization" to "Bearer $token") else emptyMap(),
        "application/json",
        JSONObject().put("sdp", sdpOffer).toString(),
    )

    /** The SDP answer (and the session id, when the JSON carries it) from a 2xx answer. */
    fun answer(status: Int, body: String): Answer {
        OpenAIRealtimeSignaling.check(status, body)
        if (body.trim().startsWith("v=")) return Answer(body, null)
        val json = runCatching { JSONObject(body) }.getOrNull()
        val sdp = json?.optJSONObject("transport")?.opt("sdp") as? String
        if (sdp == null || !sdp.trim().startsWith("v=")) {
            throw OpenAIRealtimeSignaling.SignalingException.Malformed(
                "the session endpoint answered without an SDP answer " +
                    "(expected OpenAI's { session, transport: { sdp } } JSON or the SDP text)",
            )
        }
        return Answer(sdp, json.optJSONObject("session")?.opt("id") as? String)
    }
}
