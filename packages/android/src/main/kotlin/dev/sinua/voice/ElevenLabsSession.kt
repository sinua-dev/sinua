// The I/O-free half of the native ElevenLabs `VoiceSource` (the socket glue is
// the :sinua-elevenlabs module): a port of packages/voice/src/
// ElevenLabsVoiceSource.ts's protocol and state machine, mirrored from
// packages/ios SinuaVoice/ElevenLabsSession.swift (see there for the state
// mapping). Main thread; the clock (seconds) is passed in so the 4 s thinking
// guard is testable.
package dev.sinua.voice

import org.json.JSONObject
import java.net.URLEncoder

class ElevenLabsSession(syncToAudio: Boolean = true) {
    sealed class Action {
        /** `conversation_initiation_metadata`: the negotiated formats (start the graph with these). */
        data class Metadata(val input: Pcm.AudioFormat, val output: Pcm.AudioFormat) : Action()
        data class Send(val text: String) : Action()
        class Enqueue(val samples: FloatArray, val rate: Int) : Action()
        data class ClearPlayback(val fade: Boolean) : Action()
    }

    var state: AgentState = AgentState.IDLE
        private set
    private var output = Pcm.AudioFormat(Pcm.AudioFormat.Codec.PCM, 16000)
    private var graphReady = false
    private val pendingAudio = mutableListOf<Triple<String, Boolean, JSONObject?>>() // (base64, is_final, alignment)

    /**
     * The transcript (design note 39): `chars` timing from each chunk's alignment, synced to the
     * played audio unless `syncToAudio` is false; the user's text comes final.
     */
    val transcript = TranscriptAssembler(TranscriptTiming.CHARS, syncToAudio, explicitUserEnd = true)
    var onTranscript: ((TranscriptUpdate) -> Unit)?
        get() = transcript.onUpdate
        set(value) {
            transcript.onUpdate = value
        }

    /** Audio of the current reply queued so far (ms): where the next chunk's alignment starts. */
    private var replyAudioMs = 0.0

    /** The current reply's text, shown untimed at its end if no chunk carried an alignment. */
    private var replyText = ""
    private var replyAligned = false
    private var lastInterruptEventId = 0
    private var userSpeaking = false
    private var thinkingSince = 0.0

    /** No reply in flight: `agent_response_complete` / a final chunk / an interruption since the last one began. */
    private var responseDone = true

    /** The reply was closed this turn: trailing chunks don't reopen it; the user's next turn does. */
    private var closedThisTurn = false
    private var lastAudioAt = 0.0

    var onState: ((AgentState) -> Unit)? = null
    var onInterrupt: (() -> Unit)? = null

    fun connecting(now: Double) {
        reset()
        setState(AgentState.INITIALIZING, now)
    }

    /** The graph is up at the negotiated rates: audio that arrived meanwhile is flushed now. */
    fun graphStarted(format: Pcm.AudioFormat, now: Double): List<Action> {
        output = format
        graphReady = true
        val flushed = pendingAudio.map { (b64, isFinal, alignment) -> enqueueAction(b64, isFinal, alignment, now) }
        pendingAudio.clear()
        if (state == AgentState.INITIALIZING) setState(AgentState.LISTENING, now)
        return flushed
    }

    fun reset() {
        graphReady = false
        pendingAudio.clear()
        lastInterruptEventId = 0
        userSpeaking = false
        responseDone = true
        closedThisTurn = false
    }

    fun stopped(now: Double) {
        reset()
        transcript.stop()
        setState(AgentState.IDLE, now)
    }

    /** One server frame. `playback` is the graph's current state. */
    fun handle(text: String, playback: PlaybackState, now: Double): List<Action> {
        val ev = try {
            JSONObject(text)
        } catch (_: Exception) {
            return emptyList()
        }
        when (ev.optString("type")) {
            "conversation_initiation_metadata" -> {
                val m = ev.optJSONObject("conversation_initiation_metadata_event") ?: JSONObject()
                return listOf(
                    Action.Metadata(
                        Pcm.parseAudioFormat(m.optString("user_input_audio_format", "")),
                        Pcm.parseAudioFormat(m.optString("agent_output_audio_format", "")),
                    ),
                )
            }

            "ping" -> {
                // Keep-alive: answered immediately with the same event_id, as the SDK does.
                val p = ev.optJSONObject("ping_event") ?: return emptyList()
                if (!p.has("event_id")) return emptyList()
                return listOf(
                    Action.Send(JSONObject().put("type", "pong").put("event_id", p.getInt("event_id")).toString()),
                )
            }

            "audio" -> {
                val a = ev.optJSONObject("audio_event") ?: return emptyList()
                // Late chunks of an interrupted response: dropped, the SDK's own rule.
                if (a.optInt("event_id", 0) < lastInterruptEventId) return emptyList()
                val b64 = a.optString("audio_base_64")
                if (b64.isEmpty()) return emptyList()
                val isFinal = a.optBoolean("is_final", false)
                val alignment = a.optJSONObject("alignment")
                if (!graphReady) {
                    pendingAudio += Triple(b64, isFinal, alignment)
                    return emptyList()
                }
                return listOf(enqueueAction(b64, isFinal, alignment, now))
            }

            "agent_response" -> {
                // The reply's text: a reply is under way (its audio may still be coming).
                if (!closedThisTurn) responseDone = false
                replyText = ev.optJSONObject("agent_response_event")?.optString("agent_response", "") ?: ""
                lastAudioAt = now
                return emptyList()
            }

            "agent_response_complete" -> {
                endResponse(playback, now)
                return emptyList()
            }

            "interruption" -> {
                ev.optJSONObject("interruption_event")?.let {
                    if (it.has("event_id")) {
                        lastInterruptEventId =
                            it.getInt("event_id")
                    }
                }
                if (state == AgentState.SPEAKING || playback != PlaybackState.DRAINED ||
                    pendingAudio.isNotEmpty()
                ) {
                    onInterrupt?.invoke()
                }
                transcript.cut()
                pendingAudio.clear()
                responseDone = true
                closedThisTurn = false
                setState(AgentState.LISTENING, now)
                return listOf(Action.ClearPlayback(fade = true))
            }

            "vad_score" -> {
                val score = ev.optJSONObject("vad_score_event")?.optDouble("vad_score", 0.0) ?: 0.0
                val speaking = score > VAD_THRESHOLD
                if (speaking && !userSpeaking) {
                    userSpeaking = true
                    closedThisTurn = false
                    if (state == AgentState.SPEAKING) transcript.hold()
                    if (state != AgentState.SPEAKING) setState(AgentState.LISTENING, now)
                } else if (!speaking && userSpeaking) {
                    userSpeaking = false
                    if (state == AgentState.LISTENING &&
                        playback == PlaybackState.DRAINED
                    ) {
                        setState(AgentState.THINKING, now)
                    }
                }
                return emptyList()
            }

            "user_transcript" -> {
                val e = ev.optJSONObject("user_transcription_event")
                transcript.userDone(
                    if (e != null &&
                        e.has("user_transcript")
                    ) {
                        e.getString("user_transcript")
                    } else {
                        null
                    },
                    now * 1000,
                )
                closedThisTurn = false
                if (state != AgentState.SPEAKING) setState(AgentState.THINKING, now)
                return emptyList()
            }

            // agent_response_correction / tool calls / metadata events: no bearing on the visual.
            else -> return emptyList()
        }
    }

    /**
     * 30 Hz once the graph is up: the playback-timeline gate + the thinking guard; [level] is the
     * played audio's (for the transcript).
     */
    fun tick(playback: PlaybackState, now: Double, level: Double = 0.0) {
        if (!graphReady) return
        transcript.tick(now * 1000, level, state == AgentState.SPEAKING)
        when (playback) {
            PlaybackState.AUDIBLE -> setState(AgentState.SPEAKING, now)

            PlaybackState.DRAINED -> when {
                // Drained mid-reply is a stall (more audio is coming), not the end of the turn.
                state == AgentState.SPEAKING -> setState(
                    if (responseDone) AgentState.LISTENING else AgentState.THINKING,
                    now,
                )

                state == AgentState.THINKING && responseDone && now - thinkingSince > THINKING_TIMEOUT_S -> setState(
                    AgentState.LISTENING,
                    now,
                )

                state == AgentState.THINKING && !responseDone && now - lastAudioAt > STALL_TIMEOUT_S -> {
                    responseDone = true
                    setState(AgentState.LISTENING, now)
                }
            }

            PlaybackState.QUEUED -> Unit
        }
    }

    private fun enqueueAction(b64: String, isFinal: Boolean, alignment: JSONObject?, now: Double): Action {
        val bytes = Pcm.base64Decode(b64) ?: ByteArray(0)
        val samples = if (output.codec ==
            Pcm.AudioFormat.Codec.ULAW
        ) {
            Pcm.ulawToFloat(bytes)
        } else {
            Pcm.pcm16ToFloat(bytes)
        }
        val chars = alignment?.optJSONArray("chars")
        val starts = alignment?.optJSONArray("char_start_times_ms")
        if (chars != null && chars.length() > 0 && starts != null) {
            replyAligned = true
            val durations = alignment.optJSONArray("char_durations_ms")
            val fragments = TranscriptAssembler.alignmentFragments(
                List(chars.length()) { chars.optString(it) },
                List(starts.length()) { starts.optDouble(it) },
                List(durations?.length() ?: 0) { durations!!.optDouble(it) },
                replyAudioMs,
            )
            for (f in fragments) transcript.assistantDelta(f.text, now * 1000, f.startMs, f.endMs)
        }
        replyAudioMs += samples.size * 1000.0 / output.rate
        lastAudioAt = now
        if (isFinal) {
            responseDone = true
            closedThisTurn = true
        } else if (!closedThisTurn) {
            responseDone = false
        }
        // Received, not audible yet: tick() promotes to speaking when playback reaches it.
        if (state != AgentState.SPEAKING) setState(AgentState.THINKING, now)
        return Action.Enqueue(samples, output.rate)
    }

    /**
     * `agent_response_complete`: the reply is over. With nothing left to play (a text-only
     * or empty reply, or audio already drained) the turn ends now; otherwise tick() ends it.
     */
    private fun endResponse(playback: PlaybackState, now: Double) {
        responseDone = true
        closedThisTurn = true
        // No chunk carried an alignment: the reply's text, untimed.
        if (!replyAligned && replyText.isNotEmpty()) transcript.assistantDelta(replyText, now * 1000)
        replyText = ""
        if (state == AgentState.THINKING && playback == PlaybackState.DRAINED &&
            pendingAudio.isEmpty()
        ) {
            setState(AgentState.LISTENING, now)
        }
    }

    private fun setState(s: AgentState, now: Double) {
        if (s == state) return
        // The reply is over (drained, stalled out, or cut just before): its turn ends if it was
        // heard, and the next reply's audio starts at 0 ms.
        if (s == AgentState.LISTENING || s == AgentState.IDLE) {
            transcript.speakingEnded()
            replyAudioMs = 0.0
            replyAligned = false
        }
        state = s
        if (s == AgentState.THINKING) thinkingSince = now
        onState?.invoke(s)
    }

    companion object {
        const val VAD_THRESHOLD = 0.5
        const val THINKING_TIMEOUT_S = 4.0

        /** A reply that went quiet without `agent_response_complete`: give up after this. */
        const val STALL_TIMEOUT_S = 10.0
        const val SUBPROTOCOL = "convai"

        /** An agent id -> the public-agent URL; a `wss://…` signed URL (minted by your backend) is used as-is. */
        fun endpoint(credential: String): String {
            val c = credential.trim()
            return if (c.startsWith("wss://")) {
                c
            } else {
                "wss://api.elevenlabs.io/v1/convai/conversation?agent_id=" +
                    URLEncoder.encode(c, "UTF-8")
            }
        }

        fun initMessage(overrides: JSONObject? = null): String {
            val m = JSONObject().put("type", "conversation_initiation_client_data")
            if (overrides != null) m.put("conversation_config_override", overrides)
            return m.toString()
        }

        fun micMessage(samples: FloatArray, n: Int = samples.size): String =
            JSONObject().put("user_audio_chunk", Pcm.base64Encode(Pcm.floatToPcm16(samples, n))).toString()
    }
}
