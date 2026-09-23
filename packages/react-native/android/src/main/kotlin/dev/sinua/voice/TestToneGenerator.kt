package dev.sinua.voice

import kotlin.math.PI
import kotlin.math.sin

/**
 * Sample-accurate synthesis of Web's `TestToneVoiceSource` signal: sine
 * partials 160 / 520 / 1400 Hz at gains 1 / 0.5 / 0.28 under a burst
 * envelope (0 -> 0.6 over 30 ms, linearly back to 0 by the end of a
 * 0.3-0.8 s burst, then a 0.2-0.6 s gap). Nothing is played.
 */
class TestToneGenerator(val sampleRate: Double = 48_000.0, private val random: () -> Double = { Math.random() }) {
    private var sampleIndex = 0L
    private var burstStart = 0.0
    private var burstLen = 0.0
    private var cycleLen = 0.0

    init {
        nextBurst(0.0)
    }

    private fun nextBurst(at: Double) {
        burstStart = at
        burstLen = 0.3 + random() * 0.5
        cycleLen = burstLen + 0.2 + random() * 0.4
    }

    fun next(count: Int): FloatArray = FloatArray(count) {
        val t = sampleIndex / sampleRate
        if (t - burstStart >= cycleLen) nextBurst(burstStart + cycleLen)
        val u = t - burstStart
        val env = when {
            u < 0.03 -> 0.6 * (u / 0.03)
            u < burstLen -> 0.6 * (1 - (u - 0.03) / (burstLen - 0.03))
            else -> 0.0
        }
        var s = 0.0
        for ((f, g) in PARTIALS) s += g * sin(2 * PI * f * t)
        sampleIndex++
        (s * env).toFloat()
    }

    private companion object {
        val PARTIALS = listOf(160.0 to 1.0, 520.0 to 0.5, 1400.0 to 0.28)
    }
}
