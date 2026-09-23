// OpenAI Realtime reconnect policy + transcript replay -- a port of
// packages/voice/src/realtimeReconnect.ts, mirrored from packages/ios
// SinuaVoice/RealtimeReconnect.swift. Realtime has no session resumption: a
// drop is replaced by a new session with a fresh credential, and the finalized
// transcript is replayed as `conversation.item.create`.
package dev.sinua.voice

import org.json.JSONArray
import org.json.JSONObject
import kotlin.math.min
import kotlin.math.pow
import kotlin.math.roundToInt

object RealtimeReconnect {
    const val DEFAULT_ATTEMPTS = 3
    private const val FIRST_DELAY_MS = 100.0 // LiveKit's `_interval_for_retry(0)`
    private const val BASE_DELAY_MS = 1000.0
    private const val MAX_DELAY_MS = 8000.0
    private const val JITTER = 0.2

    /** Delay before attempt `attempt` (1-based); `random` in 0..<1. */
    fun delayMs(attempt: Int, random: Double = Math.random()): Int {
        val nominal = if (attempt <= 1) FIRST_DELAY_MS else min(MAX_DELAY_MS, BASE_DELAY_MS * 2.0.pow(attempt - 2))
        val j = 1 + JITTER * (2 * random - 1)
        return maxOf(0, (nominal * j).roundToInt())
    }

    val FATAL_ERROR_CODES =
        setOf("insufficient_quota", "invalid_api_key", "account_deactivated", "billing_hard_limit_reached")

    fun isFatalError(code: String?): Boolean = code != null && code in FATAL_ERROR_CODES

    fun isRetryable(httpStatus: Int): Boolean =
        httpStatus == 408 || httpStatus == 425 || httpStatus == 429 || httpStatus >= 500
}

/** The finalized conversation, kept for replay after a reconnect. */
class TranscriptLog(private val maxChars: Int = 8000, private val maxItems: Int = 40) {
    enum class Role(val wire: String) { USER("user"), ASSISTANT("assistant") }
    data class Turn(val role: Role, val text: String)

    private var turns = mutableListOf<Turn>()

    fun add(role: Role, text: String?) {
        val t = text?.trim().orEmpty()
        if (t.isEmpty()) return
        turns += Turn(role, t)
        if (turns.size > maxItems * 2) turns = window().toMutableList()
    }

    fun clear() = turns.clear()
    val size: Int get() = turns.size

    /** The newest turns within both budgets, oldest first. */
    fun window(): List<Turn> {
        val out = mutableListOf<Turn>()
        var chars = 0
        for (turn in turns.asReversed()) {
            if (out.size >= maxItems || chars + turn.text.length > maxChars) break
            chars += turn.text.length
            out += turn
        }
        return out.asReversed()
    }

    /** `conversation.item.create` client events, in order, for a new session's data channel. */
    fun replayEvents(): List<String> = window().map { turn ->
        JSONObject()
            .put("type", "conversation.item.create")
            .put(
                "item",
                JSONObject()
                    .put("type", "message")
                    .put("role", turn.role.wire)
                    .put(
                        "content",
                        JSONArray().put(
                            JSONObject()
                                .put("type", if (turn.role == Role.USER) "input_text" else "output_text")
                                .put("text", turn.text),
                        ),
                    ),
            ).toString()
    }
}
