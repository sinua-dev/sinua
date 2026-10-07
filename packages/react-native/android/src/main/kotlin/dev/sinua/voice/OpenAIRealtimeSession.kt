// The I/O-free half of the native OpenAI Realtime `VoiceSource` (the WebRTC glue
// is the :sinua-openai module): a port of the Web Studio's audio/
// WebRTCVoiceSource.ts's event handling and tick rules, mirrored from packages/ios
// SinuaVoice/OpenAIRealtimeSession.swift (see there for the mapping). Main thread.
package dev.sinua.voice

import org.json.JSONObject

/**
 * [transcribeUser]: the user's side of transcripts needs the session's input transcription,
 * which OpenAI bills per minute; when something listens and the session has none, it is turned
 * on with this model. `null` leaves the session as your backend made it.
 */
class OpenAIRealtimeSession(
    syncToAudio: Boolean = true,
    private val transcribeUser: String? = USER_TRANSCRIPTION_MODEL,
) {
    var state: AgentState = AgentState.IDLE
        private set
    val transcript = TranscriptLog()

    /** A fatal `error.code` seen this session (e.g. `invalid_api_key`): don't reconnect. */
    var fatalCode: String? = null
        private set
    private var responseActive = false
    private var sawOutputBufferEvents = false
    private var quietFrames = 0

    /** 30 Hz ticks since the current speaking stretch began. */
    private var speakingTicks = 0

    var onState: ((AgentState) -> Unit)? = null
    var onInterrupt: (() -> Unit)? = null

    /** Events for the data channel (the glue sends them): turning the input transcription on. */
    var onSend: ((String) -> Unit)? = null

    /**
     * Live transcripts (design note 39; not the replay log above): no times from Realtime, so
     * `none` timing; the user's turn ends at `.completed`.
     */
    val captions = TranscriptAssembler(TranscriptTiming.NONE, syncToAudio, explicitUserEnd = true)

    /** Setting it also asks for the session's input transcription (see [transcribeUser]). */
    var onTranscript: ((TranscriptUpdate) -> Unit)?
        get() = captions.onUpdate
        set(value) {
            captions.onUpdate = value
            captionsWanted = value != null
            transcribeUserIfNeeded()
        }
    private var captionsWanted = false
    private var sessionCreated = false

    /** This session's input transcription is on (its own, or ours). */
    private var userTranscribed = false

    private fun transcribeUserIfNeeded() {
        val model = transcribeUser ?: return
        if (!captionsWanted || !sessionCreated || userTranscribed) return
        userTranscribed = true
        val input = JSONObject().put("transcription", JSONObject().put("model", model))
        val session = JSONObject().put("type", "realtime").put("audio", JSONObject().put("input", input))
        onSend?.invoke(JSONObject().put("type", "session.update").put("session", session).toString())
    }

    fun connecting() {
        // A new call: open transcript turns end; ids keep counting.
        captions.stop()
        sessionCreated = false
        userTranscribed = false
        responseActive = false
        sawOutputBufferEvents = false
        quietFrames = 0
        fatalCode = null
        setState(AgentState.INITIALIZING)
    }

    /** The data channel opened: the session is live. */
    fun connected() = setState(AgentState.LISTENING)

    fun stopped() {
        responseActive = false
        setState(AgentState.IDLE)
        captions.stop()
    }

    /** A server event from the `oai-events` data channel; [now] (seconds) times the transcript. */
    fun handle(text: String, now: Double = System.nanoTime() / 1e9) {
        val ev = try {
            JSONObject(text)
        } catch (_: Exception) {
            return
        }
        val ms = now * 1000
        when (ev.optString("type")) {
            "session.created" -> {
                // A new session: its own input transcription, if any, stands.
                val input = ev.optJSONObject("session")?.optJSONObject("audio")?.optJSONObject("input")
                userTranscribed = input != null && input.has("transcription") && !input.isNull("transcription")
                sessionCreated = true
                transcribeUserIfNeeded()
            }

            "input_audio_buffer.speech_started" -> {
                // Barge-in only over audible output; speech over `thinking` isn't one.
                if (state == AgentState.SPEAKING) {
                    onInterrupt?.invoke()
                    captions.cut()
                }
                setState(AgentState.LISTENING)
            }

            "response.output_audio_transcript.delta" -> ev.optString("delta").takeIf { it.isNotEmpty() }?.let {
                captions.assistantDelta(it, ms)
            }

            "conversation.item.input_audio_transcription.delta" -> ev.optString("delta").takeIf {
                it.isNotEmpty()
            }?.let {
                captions.userDelta(it, ms)
            }

            "input_audio_buffer.speech_stopped" -> setState(AgentState.THINKING)

            "response.created" -> {
                responseActive = true
                quietFrames = 0
                setState(AgentState.THINKING)
            }

            "output_audio_buffer.started" -> {
                sawOutputBufferEvents = true
                setState(AgentState.SPEAKING)
            }

            "output_audio_buffer.stopped", "output_audio_buffer.cleared" -> {
                sawOutputBufferEvents = true
                responseActive = false
                setState(AgentState.LISTENING)
            }

            "response.done" -> {
                responseActive = false
                // A response with no audio never gets output_audio_buffer.stopped -- don't hang.
                if (state == AgentState.THINKING) setState(AgentState.LISTENING)
            }

            "response.output_audio_transcript.done" -> transcript.add(
                TranscriptLog.Role.ASSISTANT,
                ev.optString("transcript", ""),
            )

            "conversation.item.input_audio_transcription.completed" -> {
                transcript.add(TranscriptLog.Role.USER, ev.optString("transcript", ""))
                captions.userDone(if (ev.has("transcript")) ev.optString("transcript") else null, ms)
            }

            "error" -> {
                val code = ev.optJSONObject("error")?.optString("code", "")?.takeIf { it.isNotEmpty() }
                if (RealtimeReconnect.isFatalError(code)) fatalCode = code
            }
        }
    }

    /** 30 Hz with the remote track's current level: the energy fallback, and the transcript. */
    fun tick(level: Double, now: Double = System.nanoTime() / 1e9) {
        captions.tick(now * 1000, level, state == AgentState.SPEAKING)
        if (state == AgentState.SPEAKING) speakingTicks++
        if (level > SPEAKING_LEVEL) {
            quietFrames = 0
            if (state == AgentState.THINKING && responseActive && !sawOutputBufferEvents) setState(AgentState.SPEAKING)
        } else if (state == AgentState.SPEAKING && !sawOutputBufferEvents && !responseActive) {
            quietFrames++
            // The Live session's adaptive tail (design note 31, V7), counted in ticks.
            if (quietFrames >=
                OpenAILiveSession.speakingTail(speakingTicks * 1000.0 / 30)
            ) {
                setState(AgentState.LISTENING)
            }
        }
    }

    private fun setState(s: AgentState) {
        if (s == state) return
        if (s == AgentState.SPEAKING) speakingTicks = 0
        // The reply is over (played out, cut just before, or a reconnect): its turn ends if it was heard.
        if (s == AgentState.LISTENING || s == AgentState.INITIALIZING || s == AgentState.IDLE) captions.speakingEnded()
        state = s
        onState?.invoke(s)
    }

    companion object {
        /** The input transcription model turned on for the user's transcript by default. */
        const val USER_TRANSCRIPTION_MODEL = "gpt-4o-mini-transcribe"
        const val SPEAKING_LEVEL = 0.05
        const val SPEAKING_TAIL_FRAMES = OpenAILiveSession.SPEAKING_TAIL_FRAMES
    }
}
