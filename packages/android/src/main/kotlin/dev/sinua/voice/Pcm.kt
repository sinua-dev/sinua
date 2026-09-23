// PCM16 little-endian <-> float, a port of packages/voice/src/pcm.ts (and
// packages/ios SinuaVoice/Pcm.swift) so the native vendor transports encode
// and decode exactly as Web does.
package dev.sinua.voice

import kotlin.math.floor
import kotlin.math.roundToLong

object Pcm {
    /** PCM16 LE bytes -> floats in -1..1 (`/32768`). A trailing odd byte is ignored. */
    fun pcm16ToFloat(bytes: ByteArray): FloatArray {
        val n = bytes.size / 2
        return FloatArray(n) {
            val v = ((bytes[2 * it].toInt() and 0xFF) or (bytes[2 * it + 1].toInt() shl 8)).toShort()
            v / 32768f
        }
    }

    /** Floats -> PCM16 LE: clamp to -1..1, `round(s * 32768)`, clamp to Int16 -- pcm.ts's `float32ToPcm16`. */
    fun floatToPcm16(samples: FloatArray, n: Int = samples.size): ByteArray {
        val out = ByteArray(n * 2)
        for (i in 0 until n) {
            val c = samples[i].toDouble().coerceIn(-1.0, 1.0)
            val v = floor(c * 32768 + 0.5).toLong().coerceIn(-32768, 32767).toInt() // JS Math.round: floor(x + 0.5)
            out[2 * i] = (v and 0xFF).toByte()
            out[2 * i + 1] = ((v shr 8) and 0xFF).toByte()
        }
        return out
    }

    /** `audio/pcm;rate=24000` -> 24000; `fallback` when absent. */
    fun parseRate(mimeType: String?, fallback: Int = 24000): Int =
        mimeType?.let { Regex("rate=(\\d+)", RegexOption.IGNORE_CASE).find(it)?.groupValues?.get(1)?.toIntOrNull() }
            ?: fallback

    /** Linear resample, only for a chunk whose rate differs from the player's fixed output rate. */
    fun resample(samples: FloatArray, from: Int, to: Int): FloatArray {
        if (from == to || from <= 0 || to <= 0 || samples.isEmpty()) return samples
        val n = maxOf(1, (samples.size.toDouble() * to / from).roundToLong().toInt())
        val step = from.toDouble() / to
        return FloatArray(n) { i ->
            val x = i * step
            val i0 = minOf(samples.size - 1, x.toInt())
            val i1 = minOf(samples.size - 1, i0 + 1)
            val t = (x - i0).toFloat()
            samples[i0] * (1 - t) + samples[i1] * t
        }
    }

    // Standard base64 (RFC 4648, padded). Not java.util.Base64: that is API 26+
    // and this library's minSdk is 24; android.util.Base64 is a stub off-device.
    private const val B64 = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/"
    private val B64_INV = IntArray(128) { -1 }.also { inv -> B64.forEachIndexed { i, c -> inv[c.code] = i } }

    fun base64Encode(bytes: ByteArray): String {
        val sb = StringBuilder((bytes.size + 2) / 3 * 4)
        var i = 0
        while (i < bytes.size) {
            val b0 = bytes[i].toInt() and 0xFF
            val b1 = if (i + 1 < bytes.size) bytes[i + 1].toInt() and 0xFF else 0
            val b2 = if (i + 2 < bytes.size) bytes[i + 2].toInt() and 0xFF else 0
            sb.append(B64[b0 shr 2]).append(B64[((b0 and 3) shl 4) or (b1 shr 4)])
            sb.append(if (i + 1 < bytes.size) B64[((b1 and 15) shl 2) or (b2 shr 6)] else '=')
            sb.append(if (i + 2 < bytes.size) B64[b2 and 63] else '=')
            i += 3
        }
        return sb.toString()
    }

    /** Decodes padded or unpadded base64; null on an invalid character. */
    fun base64Decode(text: String): ByteArray? {
        val s = text.trimEnd('=')
        val out = ByteArray(s.length * 3 / 4)
        var acc = 0
        var bits = 0
        var o = 0
        for (c in s) {
            val v = if (c.code < 128) B64_INV[c.code] else -1
            if (v < 0) return null
            acc = (acc shl 6) or v
            bits += 6
            if (bits >= 8) {
                bits -= 8
                out[o++] = ((acc shr bits) and 0xFF).toByte()
            }
        }
        return if (o == out.size) out else out.copyOf(o)
    }

    /** `pcm_16000` / `ulaw_8000` (ElevenLabs' format strings) -- pcm.ts's `parseAudioFormat`. */
    data class AudioFormat(val codec: Codec, val rate: Int) {
        enum class Codec { PCM, ULAW }
    }

    fun parseAudioFormat(
        format: String?,
        fallback: AudioFormat = AudioFormat(AudioFormat.Codec.PCM, 16000),
    ): AudioFormat {
        val m = Regex("^(pcm|ulaw)_(\\d+)$", RegexOption.IGNORE_CASE).find(format?.trim() ?: "") ?: return fallback
        val codec = if (m.groupValues[1].lowercase() == "ulaw") AudioFormat.Codec.ULAW else AudioFormat.Codec.PCM
        return AudioFormat(codec, m.groupValues[2].toIntOrNull() ?: return fallback)
    }

    private val ULAW_TABLE = intArrayOf(0, 132, 396, 924, 1980, 4092, 8316, 16764)

    /** G.711 μ-law bytes -> floats (`/32768`), pcm.ts's `ulawToFloat32` table decode. */
    fun ulawToFloat(bytes: ByteArray): FloatArray = FloatArray(bytes.size) {
        val u = bytes[it].toInt().inv() and 0xFF
        val exponent = (u shr 4) and 0x07
        var sample = ULAW_TABLE[exponent] + ((u and 0x0F) shl (exponent + 3))
        if (u and 0x80 != 0) sample = -sample
        sample / 32768f
    }
}
