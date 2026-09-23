package dev.sinua.voice

import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.floor
import kotlin.math.log10
import kotlin.math.sqrt

/**
 * Web Audio's `AnalyserNode.getByteFrequencyData`, reimplemented so native
 * analysis sees the same bytes the Web path's `AudioAnalysis` does. The
 * steps are the spec's own (WebAudio/web-audio-api `index.bs`, *FFT
 * Windowing and Smoothing over Time*): Blackman window (alpha 0.16),
 * `X[k] = (1/N) sum x[n] e^{-2 pi i k n / N}`, no smoothing (the Web path
 * sets `smoothingTimeConstant` 0), `20 log10`, then
 * `floor(255 / (maxDb - minDb) * (Y - minDb))` clipped to 0..255. Checked
 * against Chrome's own bytes (spec/voice-golden.json).
 */
class SpectrumAnalyser(val fftSize: Int = 512, val minDecibels: Double = -100.0, val maxDecibels: Double = -30.0) {
    val frequencyBinCount: Int get() = fftSize / 2
    private val fft = Fft(fftSize)
    private val window = DoubleArray(fftSize) {
        0.42 - 0.5 * cos(2 * PI * it / fftSize) + 0.08 * cos(4 * PI * it / fftSize)
    }
    private val ring = FloatArray(fftSize)
    private var writeIndex = 0
    private val re = DoubleArray(fftSize)
    private val im = DoubleArray(fftSize)

    /** Appends mono samples; only the most recent `fftSize` are analysed, as on Web. */
    fun push(samples: FloatArray, offset: Int = 0, count: Int = samples.size - offset) {
        for (i in offset until offset + count) {
            ring[writeIndex] = samples[i]
            writeIndex = (writeIndex + 1) % fftSize
        }
    }

    fun reset() {
        ring.fill(0f)
        writeIndex = 0
    }

    /** The byte spectrum of the latest `fftSize` samples, `frequencyBinCount` values 0..255. */
    fun byteFrequencyData(): IntArray {
        for (n in 0 until fftSize) {
            re[n] = ring[(writeIndex + n) % fftSize] * window[n]
            im[n] = 0.0
        }
        fft.transform(re, im)
        val scale = 255 / (maxDecibels - minDecibels)
        return IntArray(frequencyBinCount) { k ->
            val mag = sqrt(re[k] * re[k] + im[k] * im[k]) / fftSize
            val b = floor(scale * (20 * log10(mag) - minDecibels))
            if (b.isNaN() || b.isInfinite()) 0 else b.coerceIn(0.0, 255.0).toInt()
        }
    }
}
