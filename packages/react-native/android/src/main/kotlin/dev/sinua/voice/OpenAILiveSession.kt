// The I/O-free half of the native OpenAI GPT-Live `VoiceSource` (the WebRTC glue is
// :sinua-openai's OpenAILiveVoiceSource): a port of packages/voice/src/openaiLive.ts,
// mirrored from packages/ios SinuaVoice/OpenAILiveSession.swift and held to the same
// table (spec/openai-live-cases.json). Main thread.
//
// GPT-Live sends no turn events, so: the model's audio level drives `speaking` (above
// 0.05, left after ~300 ms of quiet); an open backend delegation drives `thinking` while
// the model is quiet (`session.delegation.created` or a nested `response.created`, until
// its nested terminal Responses event, the `session.commentary.appended` that delivers a
// client delegation's result -- it carries no `delegation_id`, so the oldest open client
// delegation closes -- or 30 s without news); user speech over the model's audio
// that stops it within 1 s is a barge-in (full duplex: a "mhm" under continuing speech isn't).
package dev.sinua.voice

import org.json.JSONObject

class OpenAILiveSession {
    var state: AgentState = AgentState.IDLE
        private set

    /** `session.started`'s `session.id`, once seen. */
    var sessionId: String? = null
        private set

    /** A fatal `error.code` seen this session (e.g. `insufficient_quota`): don't reconnect. */
    var fatalCode: String? = null
        private set

    /** `session.closed`'s `reason`, once seen. */
    var closedReason: String? = null
        private set
    var isStarted = false
        private set

    var onState: ((AgentState) -> Unit)? = null
    var onInterrupt: (() -> Unit)? = null
    var onClosed: ((String) -> Unit)? = null

    private var quietFrames = 0
    private var bargeInAt: Double? = null

    /** Open delegations, oldest first: id -> last news (ms) and whether the app's backend answers it. */
    private val delegations = LinkedHashMap<String, Delegation>()

    private class Delegation(val at: Double, val client: Boolean)

    fun connecting() {
        reset()
        setState(AgentState.INITIALIZING)
    }

    fun stopped() {
        reset()
        setState(AgentState.IDLE)
    }

    /** A server event from the `oai-events` data channel, at [now] ms. */
    fun handle(text: String, now: Double) {
        val ev = try {
            JSONObject(text)
        } catch (_: Exception) {
            return
        }
        when (ev.optString("type")) {
            "session.started" -> {
                ev.optJSONObject("session")?.optStringOrNull("id")?.let { sessionId = it }
                isStarted = true
                setState(AgentState.LISTENING)
            }

            "session.input_transcript.delta" ->
                if (state == AgentState.SPEAKING && bargeInAt == null) bargeInAt = now

            "session.delegation.created" -> {
                val d = ev.optJSONObject("delegation")
                d?.optStringOrNull("id")?.let { open(it, now, d.optStringOrNull("target") == "client") }
            }

            "response.event" -> {
                val id = ev.optStringOrNull("delegation_id") ?: return
                val inner = ev.optJSONObject("event")?.optStringOrNull("type") ?: return
                if (inner in TERMINAL_RESPONSE_EVENTS) {
                    close(id)
                } else if (inner == "response.created" || delegations.containsKey(id)) {
                    open(id, now, client = false)
                }
            }

            // A client delegation's result was delivered: the named one, else the oldest owed.
            "session.commentary.appended" ->
                (ev.optStringOrNull("delegation_id") ?: delegations.entries.firstOrNull { it.value.client }?.key)
                    ?.let { close(it) }

            "session.closed" -> {
                val reason = ev.optStringOrNull("reason") ?: "unknown"
                closedReason = reason
                onClosed?.invoke(reason)
            }

            "error" -> {
                val code = ev.optJSONObject("error")?.optStringOrNull("code")
                if (RealtimeReconnect.isFatalError(code)) fatalCode = code
            }
        }
    }

    /** 30 Hz with the remote track's current level, at [now] ms. */
    fun tick(level: Double, now: Double) {
        if (!isStarted) return
        delegations.entries.removeAll { now - it.value.at > DELEGATION_TIMEOUT_MS }
        if (level > SPEAKING_LEVEL) {
            quietFrames = 0
            setState(AgentState.SPEAKING)
        } else if (state == AgentState.SPEAKING) {
            quietFrames++
            val userSpoke = bargeInAt?.let { now - it <= BARGE_IN_WINDOW_MS } ?: false
            if (quietFrames < if (userSpoke) BARGE_IN_TAIL_FRAMES else SPEAKING_TAIL_FRAMES) return
            val armed = bargeInAt
            bargeInAt = null
            quietFrames = 0
            setState(if (delegations.isEmpty()) AgentState.LISTENING else AgentState.THINKING)
            if (armed != null && now - armed <= BARGE_IN_WINDOW_MS) onInterrupt?.invoke()
        } else {
            settle()
        }
    }

    private fun open(id: String, now: Double, client: Boolean) {
        delegations[id] = Delegation(now, delegations[id]?.client ?: client)
        settle()
    }

    private fun close(id: String) {
        if (delegations.remove(id) != null) settle()
    }

    /** Listening <-> thinking by open delegations; speaking is left alone. */
    private fun settle() {
        if (!isStarted || state == AgentState.SPEAKING) return
        setState(if (delegations.isEmpty()) AgentState.LISTENING else AgentState.THINKING)
    }

    private fun reset() {
        isStarted = false
        sessionId = null
        fatalCode = null
        closedReason = null
        quietFrames = 0
        bargeInAt = null
        delegations.clear()
    }

    private fun setState(s: AgentState) {
        if (s == state) return
        state = s
        onState?.invoke(s)
    }

    companion object {
        const val SPEAKING_LEVEL = 0.05

        /**
         * Speaking ends after ~1 s of quiet, so a pause between phrases doesn't flip the state
         * (design note 30, V3); after ~300 ms when the user just spoke (a barge-in stays instant).
         */
        const val SPEAKING_TAIL_FRAMES = 30
        const val BARGE_IN_TAIL_FRAMES = 9
        const val BARGE_IN_WINDOW_MS = 1000.0
        const val DELEGATION_TIMEOUT_MS = 30_000.0
        private val TERMINAL_RESPONSE_EVENTS =
            setOf("response.completed", "response.failed", "response.incomplete", "response.cancelled")
    }
}

/** A string field, or null when it's absent, not a string, or empty. */
private fun JSONObject.optStringOrNull(key: String): String? = (opt(key) as? String)?.takeIf { it.isNotEmpty() }
