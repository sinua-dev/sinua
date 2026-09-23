// The I/O-free half of the native OpenAI Realtime `VoiceSource` (the WebRTC glue
// is the :sinua-openai module): a port of the Web Studio's audio/
// WebRTCVoiceSource.ts's event handling and tick rules, mirrored from packages/ios
// SinuaVoice/OpenAIRealtimeSession.swift (see there for the mapping). Main thread.
package dev.sinua.voice

import org.json.JSONObject

class OpenAIRealtimeSession {
    var state: AgentState = AgentState.IDLE
        private set
    val transcript = TranscriptLog()

    /** A fatal `error.code` seen this session (e.g. `invalid_api_key`): don't reconnect. */
    var fatalCode: String? = null
        private set
    private var responseActive = false
    private var sawOutputBufferEvents = false
    private var quietFrames = 0

    var onState: ((AgentState) -> Unit)? = null
    var onInterrupt: (() -> Unit)? = null

    fun connecting() {
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
    }

    /** A server event from the `oai-events` data channel. */
    fun handle(text: String) {
        val ev = try {
            JSONObject(text)
        } catch (_: Exception) {
            return
        }
        when (ev.optString("type")) {
            "input_audio_buffer.speech_started" -> {
                // Barge-in only over audible output; speech over `thinking` isn't one.
                if (state == AgentState.SPEAKING) onInterrupt?.invoke()
                setState(AgentState.LISTENING)
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

            "conversation.item.input_audio_transcription.completed" -> transcript.add(
                TranscriptLog.Role.USER,
                ev.optString("transcript", ""),
            )

            "error" -> {
                val code = ev.optJSONObject("error")?.optString("code", "")?.takeIf { it.isNotEmpty() }
                if (RealtimeReconnect.isFatalError(code)) fatalCode = code
            }
        }
    }

    /** 30 Hz with the remote track's current level: the energy fallback only. */
    fun tick(level: Double) {
        if (level > SPEAKING_LEVEL) {
            quietFrames = 0
            if (state == AgentState.THINKING && responseActive && !sawOutputBufferEvents) setState(AgentState.SPEAKING)
        } else if (state == AgentState.SPEAKING && !sawOutputBufferEvents && !responseActive) {
            quietFrames++
            if (quietFrames >= SPEAKING_TAIL_FRAMES) setState(AgentState.LISTENING)
        }
    }

    private fun setState(s: AgentState) {
        if (s == state) return
        state = s
        onState?.invoke(s)
    }

    companion object {
        const val SPEAKING_LEVEL = 0.05
        const val SPEAKING_TAIL_FRAMES = 9 // ~300 ms at 30 Hz
    }
}
