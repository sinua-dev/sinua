package dev.sinua.voice

import kotlin.math.exp
import kotlin.math.floor
import kotlin.math.ln
import kotlin.math.max
import kotlin.math.min
import kotlin.math.sqrt

/**
 * Byte spectrum -> [VoiceMetrics], a line-for-line port of Web's
 * packages/voice/src/analysis.ts (see its doc comment for why each step
 * exists): the voice-relevant bin range [5 %, 40 %), log-spaced band edges,
 * sqrt compression, asymmetric attack/release, level = the loudest band.
 */
class AudioAnalysis(
    val bandCount: Int = 16,
    private val attack: Double = 0.7,
    private val release: Double = 0.12,
    private val loFrac: Double = 0.05,
    private val hiFrac: Double = 0.4,
) {
    private var levelState = 0.0
    private val bandState = DoubleArray(bandCount)

    /** One update tick over a byte spectrum ([SpectrumAnalyser.byteFrequencyData]). */
    fun read(data: IntArray): VoiceMetrics {
        val count = data.size
        val lo = max(1, floor(count * loFrac).toInt())
        val hi = max(lo + bandCount, floor(count * hiFrac).toInt())
        val logLo = ln(lo.toDouble())
        val logHi = ln(hi.toDouble())
        // JS Math.round: floor(x + 0.5).
        val edges =
            IntArray(bandCount + 1) { b ->
                floor(exp(logLo + (logHi - logLo) * (b.toDouble() / bandCount)) + 0.5).toInt()
            }

        val targets = DoubleArray(bandCount)
        for (b in 0 until bandCount) {
            val start = edges[b]
            val end = max(start + 1, edges[b + 1])
            var sum = 0.0
            var n = 0
            var i = start
            while (i < end && i < count) {
                sum += data[i]
                n++
                i++
            }
            val raw = if (n > 0) sum / n / 255 else 0.0
            targets[b] = sqrt(max(0.0, min(1.0, raw)))
        }
        for (b in 0 until bandCount) {
            val rate = if (targets[b] > bandState[b]) attack else release
            bandState[b] += (targets[b] - bandState[b]) * rate
        }
        val levelTarget = targets.fold(0.0) { a, v -> max(a, v) }
        val levelRate = if (levelTarget > levelState) attack else release
        levelState += (levelTarget - levelState) * levelRate
        return VoiceMetrics(levelState, bandState.toList())
    }

    fun reset() {
        levelState = 0.0
        bandState.fill(0.0)
    }
}
