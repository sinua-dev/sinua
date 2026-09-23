// The voice contract, mirrored from packages/core/src/voice.ts (Web) and
// packages/ios SinuaVoice. Same vocabulary, same encodings, so a native
// app feeds the engine the exact opts keys the Web path does. See
// docs/audio-pipeline.md, *Native*.
package dev.sinua.voice

/** LiveKit Agents' `AgentState` vocabulary -- same strings as Web's `AgentState`. */
enum class AgentState(val wire: String, val voiceStateCode: Double) {
    // voiceStateCode mirrors `VoiceState` in crates/core_engine/src/signal/modes/bar.rs.
    IDLE("idle", 0.0),
    INITIALIZING("initializing", 1.0),
    LISTENING("listening", 2.0),
    THINKING("thinking", 3.0),
    SPEAKING("speaking", 4.0),
    ;

    companion object {
        fun fromWire(s: String): AgentState? = entries.firstOrNull { it.wire == s }
    }
}

/** A smoothed, normalized reading: overall level and per-band levels, 0..1, low frequency first. */
data class VoiceMetrics(val level: Double, val bands: List<Double>) {
    companion object {
        val SILENT = VoiceMetrics(0.0, emptyList())
    }
}

/**
 * One source of voice readings (a mic, a test tone, later a vendor
 * transport). Callbacks arrive on the main thread.
 */
interface VoiceSource {
    /** Starts the source; throws if it can't (e.g. `SecurityException` without RECORD_AUDIO). */
    fun connect()
    fun disconnect()
    fun onMetrics(cb: (VoiceMetrics) -> Unit)
    fun onStateChange(cb: (AgentState) -> Unit)

    /** Optional barge-in moment (see Web's `VoiceSource.onInterrupt`); a no-op by default. */
    fun onInterrupt(cb: () -> Unit) {}
}

/** `primitives::audio_band`'s cap: keys `audioBand0`..`audioBand15` exist, nothing beyond. */
const val MAX_AUDIO_BANDS = 16

/** The mic/test-tone energy heuristic threshold -- Web's 0.08. */
internal const val SPEAKING_LEVEL = 0.08
