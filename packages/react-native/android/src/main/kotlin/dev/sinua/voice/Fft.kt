package dev.sinua.voice

import kotlin.math.PI
import kotlin.math.cos
import kotlin.math.sin

/**
 * In-place iterative radix-2 complex FFT (forward, unnormalized). Android
 * has no first-party FFT for mic audio -- `android.media.audiofx.Visualizer`
 * only taps an *output* session -- and this project's library takes no
 * third-party dependency for ~40 lines of textbook Cooley-Tukey.
 */
internal class Fft(val size: Int) {
    private val cosTable = DoubleArray(size / 2) { cos(-2 * PI * it / size) }
    private val sinTable = DoubleArray(size / 2) { sin(-2 * PI * it / size) }
    private val levels = Integer.numberOfTrailingZeros(size)

    init {
        require(size >= 2 && size and (size - 1) == 0) { "FFT size must be a power of two" }
    }

    fun transform(re: DoubleArray, im: DoubleArray) {
        // Bit-reversal permutation.
        for (i in 0 until size) {
            val j = Integer.reverse(i) ushr (32 - levels)
            if (j > i) {
                var t = re[i]
                re[i] = re[j]
                re[j] = t
                t = im[i]
                im[i] = im[j]
                im[j] = t
            }
        }
        var len = 2
        while (len <= size) {
            val half = len / 2
            val step = size / len
            var i = 0
            while (i < size) {
                var k = 0
                for (j in i until i + half) {
                    val l = j + half
                    val tre = re[l] * cosTable[k] - im[l] * sinTable[k]
                    val tim = re[l] * sinTable[k] + im[l] * cosTable[k]
                    re[l] = re[j] - tre
                    im[l] = im[j] - tim
                    re[j] += tre
                    im[j] += tim
                    k += step
                }
                i += len
            }
            len *= 2
        }
    }
}
