// The I/O-free half of the native Gemini Live `VoiceSource` (the socket glue is
// the :sinua-gemini module): a port of the Web Studio's audio/
// PCMStreamVoiceSource.ts's protocol and state machine, mirrored from packages/ios
// SinuaVoice/GeminiLiveSession.swift (see there for the state mapping). Wire
// format per ai.google.dev/api/live, checked 2026-09-19. Main thread.
package dev.sinua.voice

import org.json.JSONArray
import org.json.JSONObject

class GeminiLiveSession(
    val model: String = DEFAULT_MODEL,
    val instructions: String? = null,
    syncToAudio: Boolean = true,
) {
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

    /**
     * The transcript (design note 39): no times from Gemini, so `none` timing, paced over the
     * reply's received audio unless `syncToAudio` is false.
     */
    val transcript = TranscriptAssembler(TranscriptTiming.NONE, syncToAudio)
    var onTranscript: ((TranscriptUpdate) -> Unit)?
        get() = transcript.onUpdate
        set(value) {
            transcript.onUpdate = value
        }

    fun setupMessage(): String {
        val setup = JSONObject()
            .put("model", if (model.startsWith("models/")) model else "models/$model")
            .put("generationConfig", JSONObject().put("responseModalities", JSONArray().put("AUDIO")))
            // Gemini's own ASR of the user: the vendor-side "the user is being heard" signal.
            .put("inputAudioTranscription", JSONObject())
            // The model's own words, for transcripts (the same Live session; no extra request).
            .put("outputAudioTranscription", JSONObject())
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
        transcript.stop()
        resumptionHandle = null
        setupDone = false
        generationDone = true
        setState(AgentState.IDLE)
    }

    /**
     * One server frame (text or decoded binary). `playback` is the graph's current state; [now]
     * (seconds) times the transcript.
     */
    fun handle(text: String, playback: PlaybackState, now: Double = System.nanoTime() / 1e9): List<Action> {
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
                transcript.cut()
                actions += Action.ClearPlayback(fade = true)
                generationDone = true
                setState(AgentState.LISTENING)
            }
            if ((sc.has("interimInputTranscription") || sc.has("inputTranscription")) && state != AgentState.SPEAKING) {
                setState(AgentState.LISTENING)
            }
            sc.optJSONObject("inputTranscription")?.optString("text")?.takeIf { it.isNotEmpty() }?.let {
                if (state == AgentState.SPEAKING) transcript.hold()
                transcript.userDelta(it, now * 1000)
            }
            sc.optJSONObject("outputTranscription")?.optString("text")?.takeIf { it.isNotEmpty() }?.let {
                transcript.assistantDelta(it, now * 1000)
            }
            val parts = sc.optJSONObject("modelTurn")?.optJSONArray("parts")
            for (i in 0 until (parts?.length() ?: 0)) {
                val inline = parts!!.optJSONObject(i)?.optJSONObject("inlineData") ?: continue
                val mime = inline.optString("mimeType")
                val b64 = inline.optString("data")
                if (!mime.startsWith("audio/pcm") || b64.isEmpty()) continue
                val bytes = Pcm.base64Decode(b64) ?: continue
                val samples = Pcm.pcm16ToFloat(bytes)
                val rate = Pcm.parseRate(mime, OUTPUT_RATE)
                actions += Action.Enqueue(samples, rate)
                transcript.assistantAudio(samples.size * 1000.0 / rate)
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

    /** 30 Hz, only while connected: the playback-timeline gate; [level] is the played audio's (for the transcript). */
    fun tick(playback: PlaybackState, level: Double = 0.0, now: Double = System.nanoTime() / 1e9) {
        if (!setupDone) return
        transcript.tick(now * 1000, level, state == AgentState.SPEAKING)
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
        // The reply is over (drained, cut just before, or a reconnect): its turn ends if it was heard.
        if (s == AgentState.LISTENING || s == AgentState.INITIALIZING ||
            s == AgentState.IDLE
        ) {
            transcript.speakingEnded()
        }
        state = s
        onState?.invoke(s)
    }

    companion object {
        const val INPUT_RATE = 16000
        const val OUTPUT_RATE = 24000
        const val DEFAULT_MODEL = "gemini-3.8-live"

        /**
         * An `auth_tokens/…` ephemeral token (minted by the app's backend) -> the
         * Constrained method with `Authorization: Token …` -- a header, as Google's
         * python-genai `live.py` connects, so no secret is in the URL. There is no
         * raw-key path: `GeminiLiveVoiceSource` refuses anything else.
         */
        fun endpoint(credential: String): Endpoint {
            val base = "wss://generativelanguage.googleapis.com/ws/" +
                "google.ai.generativelanguage.v1beta.GenerativeService."
            return Endpoint(
                base + "BidiGenerateContentConstrained",
                mapOf("Authorization" to "Token ${credential.trim()}"),
            )
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
