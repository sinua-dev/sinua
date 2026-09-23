// The I/O-free half of the native Gemini Live `VoiceSource` (the socket glue is
// the :sinua-gemini module): a port of the Web Studio's audio/
// PCMStreamVoiceSource.ts's protocol and state machine, mirrored from packages/ios
// SinuaVoice/GeminiLiveSession.swift (see there for the state mapping). Wire
// format per ai.google.dev/api/live, checked 2026-09-19. Main thread.
package dev.sinua.voice

import org.json.JSONArray
import org.json.JSONObject

class GeminiLiveSession(val model: String = DEFAULT_MODEL, val instructions: String? = null) {
    /** Side effects the glue performs on the audio graph / socket. */
    sealed class Action {
        object SetupComplete : Action()
        class Enqueue(val samples: FloatArray, val rate: Int) : Action()
        data class ClearPlayback(val fade: Boolean) : Action()
        data class Reconnect(val reason: String) : Action()
        data class ServerError(val message: String) : Action()
    }

    data class Endpoint(val url: String, val headers: Map<String, String>)

    var state: AgentState = AgentState.IDLE
        private set
    var resumptionHandle: String? = null
        private set
    private var generationDone = true
    private var setupDone = false

    var onState: ((AgentState) -> Unit)? = null
    var onInterrupt: (() -> Unit)? = null

    fun setupMessage(): String {
        val setup = JSONObject()
            .put("model", if (model.startsWith("models/")) model else "models/$model")
            .put("generationConfig", JSONObject().put("responseModalities", JSONArray().put("AUDIO")))
            // Gemini's own ASR of the user: the vendor-side "the user is being heard" signal.
            .put("inputAudioTranscription", JSONObject())
            .put("sessionResumption", resumptionHandle?.let { JSONObject().put("handle", it) } ?: JSONObject())
            .put("contextWindowCompression", JSONObject().put("slidingWindow", JSONObject()))
        instructions?.let {
            setup.put("systemInstruction", JSONObject().put("parts", JSONArray().put(JSONObject().put("text", it))))
        }
        return JSONObject().put("setup", setup).toString()
    }

    /** Opening (or reopening) the socket. */
    fun connecting() {
        setupDone = false
        generationDone = true
        setState(AgentState.INITIALIZING)
    }

    /** A fresh `connect()` starts a fresh session (Web's teardown clears the handle). */
    fun reset() {
        resumptionHandle = null
        setupDone = false
        generationDone = true
        setState(AgentState.IDLE)
    }

    /** One server frame (text or decoded binary). `playback` is the graph's current state. */
    fun handle(text: String, playback: PlaybackState): List<Action> {
        val msg = try {
            JSONObject(text)
        } catch (_: Exception) {
            return emptyList()
        }
        val actions = mutableListOf<Action>()
        if (msg.has("setupComplete") && !setupDone) {
            setupDone = true
            setState(AgentState.LISTENING)
            return listOf(Action.SetupComplete)
        }
        msg.optJSONObject("serverContent")?.let { sc ->
            if (sc.optBoolean("interrupted")) {
                // The Live guide: stop playing and clear the queue. A barge-in only if there was output to cut.
                if (state == AgentState.SPEAKING || playback != PlaybackState.DRAINED) onInterrupt?.invoke()
                actions += Action.ClearPlayback(fade = true)
                generationDone = true
                setState(AgentState.LISTENING)
            }
            if ((sc.has("interimInputTranscription") || sc.has("inputTranscription")) && state != AgentState.SPEAKING) {
                setState(AgentState.LISTENING)
            }
            val parts = sc.optJSONObject("modelTurn")?.optJSONArray("parts")
            for (i in 0 until (parts?.length() ?: 0)) {
                val inline = parts!!.optJSONObject(i)?.optJSONObject("inlineData") ?: continue
                val mime = inline.optString("mimeType")
                val b64 = inline.optString("data")
                if (!mime.startsWith("audio/pcm") || b64.isEmpty()) continue
                val bytes = Pcm.base64Decode(b64) ?: continue
                actions += Action.Enqueue(Pcm.pcm16ToFloat(bytes), Pcm.parseRate(mime, OUTPUT_RATE))
                generationDone = false
                // Received, not audible yet: tick() promotes to speaking when playback reaches it.
                if (state != AgentState.SPEAKING) setState(AgentState.THINKING)
            }
            if (sc.optBoolean("generationComplete") || sc.optBoolean("turnComplete") ||
                sc.optBoolean("waitingForInput")
            ) {
                generationDone = true
            }
        }
        msg.optJSONObject("sessionResumptionUpdate")?.let { upd ->
            val h = upd.optString("newHandle")
            if (upd.optBoolean("resumable") && h.isNotEmpty()) resumptionHandle = h
        }
        msg.optJSONObject("goAway")?.let {
            actions +=
                Action.Reconnect("goAway (timeLeft ${it.optString("timeLeft", "?")})")
        }
        if (msg.has("error")) actions += Action.ServerError(msg.get("error").toString())
        return actions
    }

    /** 30 Hz, only while connected: the playback-timeline gate. */
    fun tick(playback: PlaybackState) {
        if (!setupDone) return
        when (playback) {
            PlaybackState.AUDIBLE -> setState(AgentState.SPEAKING)

            PlaybackState.DRAINED -> if (state == AgentState.SPEAKING || state == AgentState.THINKING) {
                setState(if (generationDone) AgentState.LISTENING else AgentState.THINKING)
            }

            PlaybackState.QUEUED -> Unit
        }
    }

    private fun setState(s: AgentState) {
        if (s == state) return
        state = s
        onState?.invoke(s)
    }

    companion object {
        const val INPUT_RATE = 16000
        const val OUTPUT_RATE = 24000
        const val DEFAULT_MODEL = "gemini-3.8-live"

        /**
         * `auth_tokens/…` (ephemeral, minted by the app's backend) -> the Constrained
         * method with `Authorization: Token …`; anything else is a raw API key (dev
         * only) in `x-goog-api-key`. Headers, as Google's python-genai `live.py`
         * connects, so no secret is in the URL.
         */
        fun endpoint(credential: String): Endpoint {
            val c = credential.trim()
            val base = "wss://generativelanguage.googleapis.com/ws/" +
                "google.ai.generativelanguage.v1beta.GenerativeService."
            return if (c.startsWith("auth_tokens/")) {
                Endpoint(base + "BidiGenerateContentConstrained", mapOf("Authorization" to "Token $c"))
            } else {
                Endpoint(base + "BidiGenerateContent", mapOf("x-goog-api-key" to c))
            }
        }

        fun micMessage(samples: FloatArray, n: Int = samples.size): String = JSONObject().put(
            "realtimeInput",
            JSONObject().put(
                "audio",
                JSONObject()
                    .put("data", Pcm.base64Encode(Pcm.floatToPcm16(samples, n)))
                    .put("mimeType", "audio/pcm;rate=$INPUT_RATE"),
            ),
        ).toString()
    }
}
