package dev.sinua.voice

import kotlin.math.max
import kotlin.math.min

/** The barge-in flash (see Web's `InterruptOptions`). */
data class InterruptOptions(
    /** Seconds `interruptAge` keeps being emitted after the moment. */
    val window: Double = 1.0,
    val duration: Double? = null,
    val strength: Double? = null,
    val tint: Double? = null,
    val hue: Double? = null,
)

/** Same options and defaults as Web's `VoiceOverridesOptions`. */
data class VoiceOverridesOptions(
    /** `audioStrength` (orb breathing pulse); 0 disables. */
    val audioStrength: Double = 0.18,
    /** `audioLevel` easing per second; `Double.POSITIVE_INFINITY` = raw. */
    val levelEaseRate: Double = 7.0,
    /** `audioBand*` easing per second; `Double.POSITIVE_INFINITY` = raw. */
    val bandEaseRate: Double = 24.0,
    /** Scrolling-style history size; null = off. */
    val historyCount: Int? = null,
    val historyHz: Double = 12.0,
    /** null disables `interruptAge`. */
    val interrupt: InterruptOptions? = InterruptOptions(),
    val muted: Boolean = false,
    val mutedTint: Double? = null,
    val mutedHue: Double? = null,
)

/** Pure, stateless form: one reading + one state -> overrides (no easing, no history, no interrupt). */
fun voiceOverrides(
    metrics: VoiceMetrics,
    state: AgentState,
    options: VoiceOverridesOptions = VoiceOverridesOptions(),
): Map<String, Double> = buildOverrides(metrics.level, metrics.bands, state, options, options.muted, null, null)

/**
 * Port of packages/core/src/voice.ts's `VoiceOverrides`: a live voice
 * reading -> the exact `Map<String, Double>` `frameWithOverrides` takes,
 * with the Web Studio's easing, the barge-in age and the scrolling-style
 * history. Clock-free (dt-driven); tested against Web's own output
 * (spec/voice-golden.json).
 */
class VoiceOverrides(private val options: VoiceOverridesOptions = VoiceOverridesOptions()) {
    private var history: ArrayDeque<Double>? = options.historyCount?.let { n -> ArrayDeque(List(max(1, n)) { 0.0 }) }
    private var historyAcc = 0.0
    private var historyPeak = 0.0
    private var historyPhase = 0.0
    private var interruptAge: Double? = null
    private var interruptFresh = false
    private var level = 0.0
    private var bands = DoubleArray(0)

    var metrics: VoiceMetrics = VoiceMetrics.SILENT
        private set
    var state: AgentState = AgentState.IDLE
        private set

    /** The `muted` cue; flip it live. */
    var muted: Boolean = options.muted

    fun push(metrics: VoiceMetrics) {
        this.metrics = metrics
    }

    fun setState(state: AgentState) {
        this.state = state
    }

    /** Marks the barge-in moment now (see voice.ts `interrupt()`). */
    fun interrupt() {
        if (options.interrupt == null) return
        interruptAge = 0.0
        interruptFresh = true
    }

    fun setHistoryCount(count: Int) {
        val n = max(1, count)
        val h = history ?: ArrayDeque<Double>().also { history = it }
        while (h.size > n) h.removeFirst()
        while (h.size < n) h.addFirst(0.0)
    }

    fun reset() {
        metrics = VoiceMetrics.SILENT
        level = 0.0
        bands = DoubleArray(0)
        history?.let { h -> for (i in h.indices) h[i] = 0.0 }
        historyAcc = 0.0
        historyPeak = 0.0
        historyPhase = 0.0
        interruptAge = null
        interruptFresh = false
    }

    /** Call once per rendered frame with the seconds since the previous frame. */
    fun overrides(dtSeconds: Double): Map<String, Double> {
        val dt = max(0.0, min(dtSeconds, MAX_DT_S))

        var interrupt: Double? = null
        val io = options.interrupt
        val age = interruptAge
        if (age != null && io != null) {
            var a = age
            if (interruptFresh) interruptFresh = false else a += if (dtSeconds.isFinite()) max(0.0, dtSeconds) else 0.0
            if (a >= io.window) {
                interruptAge = null
            } else {
                interruptAge = a
                interrupt = a
            }
        }

        level += (metrics.level - level) * easeK(options.levelEaseRate, dt)
        val raw = metrics.bands
        if (bands.size != raw.size) {
            bands = raw.toDoubleArray()
        } else {
            val k = easeK(options.bandEaseRate, dt)
            for (i in raw.indices) bands[i] += (raw[i] - bands[i]) * k
        }

        val h = history
        var hist: Pair<List<Double>, Double>? = null
        if (h != null) {
            val interval = 1 / options.historyHz
            historyAcc += dt
            historyPeak = max(historyPeak, metrics.level)
            while (historyAcc >= interval) {
                h.addLast(historyPeak)
                h.removeFirst()
                historyPeak = 0.0
                historyAcc -= interval
            }
            historyPhase = historyAcc / interval
            hist = h.toList() to historyPhase
        }
        return buildOverrides(level, bands.toList(), state, options, muted, hist, interrupt)
    }

    companion object {
        private const val MAX_DT_S = 0.1

        /**
         * Subscribes to [source]'s metrics, state and interrupt callbacks. A source holds one
         * callback of each kind -- read [metrics]/[state] here instead of subscribing again.
         */
        fun bind(source: VoiceSource, options: VoiceOverridesOptions = VoiceOverridesOptions()): VoiceOverrides {
            val v = VoiceOverrides(options)
            source.onMetrics { v.push(it) }
            source.onInterrupt { v.interrupt() }
            source.onStateChange {
                v.setState(it)
                if (it == AgentState.IDLE) v.reset()
            }
            return v
        }
    }
}

private fun easeK(rate: Double, dt: Double): Double = if (dt > 0) min(1.0, rate * dt) else 0.0

private fun clamp01(v: Double): Double = if (v.isNaN()) 0.0 else v.coerceIn(0.0, 1.0)

private fun buildOverrides(
    level: Double,
    bands: List<Double>,
    state: AgentState,
    options: VoiceOverridesOptions,
    muted: Boolean,
    history: Pair<List<Double>, Double>?,
    interruptAge: Double?,
): Map<String, Double> {
    val out = HashMap<String, Double>()
    out["audioLevel"] = clamp01(level)
    out["audioStrength"] = options.audioStrength
    out["voiceStateCode"] = state.voiceStateCode
    val n = min(bands.size, MAX_AUDIO_BANDS)
    out["audioBandCount"] = n.toDouble()
    for (i in 0 until n) out["audioBand$i"] = clamp01(bands[i])
    if (history != null) {
        out["historyCount"] = history.first.size.toDouble()
        history.first.forEachIndexed { i, v -> out["history$i"] = clamp01(v) }
        out["historyPhase"] = history.second
    }
    val io = options.interrupt
    if (interruptAge != null && io != null) {
        out["interruptAge"] = interruptAge
        io.duration?.let { out["interruptDuration"] = it }
        io.strength?.let { out["interruptStrength"] = it }
        io.tint?.let { out["interruptTint"] = it }
        io.hue?.let { out["interruptHue"] = it }
    }
    if (muted) {
        out["muted"] = 1.0
        options.mutedTint?.let { out["mutedTint"] = it }
        options.mutedHue?.let { out["mutedHue"] = it }
    }
    return out
}
