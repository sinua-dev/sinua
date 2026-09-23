package dev.sinua.voice

import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.Base64
import kotlin.math.abs
import kotlin.math.max

/**
 * Parity against spec/voice-golden.json (packages/core/scripts/gen-voice-golden.mjs):
 * Chrome's own AnalyserNode bytes + the Web Studio's analysis.ts metrics for a fixed
 * signal, and Web VoiceOverrides' maps for a scripted feed. Plain JVM tests -- the
 * pure parts need no device. Gradle runs them with the module dir (packages/android)
 * as the working directory.
 */
class VoiceGoldenTest {
    private val golden: JSONObject by lazy { JSONObject(File("../../spec/voice-golden.json").readText()) }

    @Test
    fun spectrumAndAnalysisMatchChromeAndWeb() {
        val a = golden.getJSONObject("analysis")
        val bytes = Base64.getDecoder().decode(a.getString("samplesF32Base64"))
        val fb = ByteBuffer.wrap(bytes).order(ByteOrder.LITTLE_ENDIAN).asFloatBuffer()
        val samples = FloatArray(fb.remaining()).also { fb.get(it) }
        val frames = a.getJSONArray("frames")
        val spectrum = SpectrumAnalyser(a.getInt("fftSize"))
        val analysis = AudioAnalysis()
        var pos = 0
        var offByOne = 0
        var total = 0
        var maxErr = 0.0
        for (i in 0 until frames.length()) {
            val f = frames.getJSONObject(i)
            val end = f.getInt("end")
            spectrum.push(samples, pos, end - pos)
            pos = end
            val got = spectrum.byteFrequencyData()
            val want = f.getJSONArray("bytes")
            for (k in 0 until want.length()) {
                val d = abs(got[k] - want.getInt(k))
                assertTrue("end $end bin $k: ${got[k]} vs ${want.getInt(k)}", d <= 1)
                if (d > 0) offByOne++
                total++
            }
            val m = analysis.read(got)
            val level = f.getDouble("level")
            maxErr = max(maxErr, abs(m.level - level))
            val bands = f.getJSONArray("bands")
            for (b in 0 until bands.length()) maxErr = max(maxErr, abs(m.bands[b] - bands.getDouble(b)))
            assertEquals("state heuristic at end $end", level > SPEAKING_LEVEL, m.level > SPEAKING_LEVEL)
        }
        assertTrue("$offByOne/$total bins off by one", offByOne.toDouble() / total < 0.005)
        assertTrue("max metric err $maxErr", maxErr < 0.01)
        println("dev.sinua.voice parity: $offByOne/$total bins off by one, max metric err $maxErr")
    }

    @Test
    fun analysisOnChromeBytesIsExact() {
        val frames = golden.getJSONObject("analysis").getJSONArray("frames")
        val analysis = AudioAnalysis()
        for (i in 0 until frames.length()) {
            val f = frames.getJSONObject(i)
            val bytes = f.getJSONArray("bytes").let { arr -> IntArray(arr.length()) { arr.getInt(it) } }
            val m = analysis.read(bytes)
            assertEquals(f.getDouble("level"), m.level, 1e-12)
            val bands = f.getJSONArray("bands")
            for (b in 0 until bands.length()) assertEquals(bands.getDouble(b), m.bands[b], 1e-12)
        }
    }

    private fun num(v: Any?): Double? = when (v) {
        "inf" -> Double.POSITIVE_INFINITY
        is Number -> v.toDouble()
        else -> null
    }

    @Test
    fun voiceOverridesMatchWeb() {
        val o = golden.getJSONObject("overrides")
        val steps = o.getJSONArray("steps")
        val configs = o.getJSONObject("configs")
        val expected = o.getJSONObject("expected")
        for (name in configs.keys()) {
            val c = configs.getJSONObject(name)
            val interrupt = when (val i = c.opt("interrupt")) {
                null -> InterruptOptions()

                is JSONObject -> InterruptOptions(
                    window = num(i.opt("window")) ?: 1.0,
                    duration = num(i.opt("duration")),
                    strength = num(i.opt("strength")),
                    tint = num(i.opt("tint")),
                    hue = num(i.opt("hue")),
                )

                else -> null // false
            }
            val opts = VoiceOverridesOptions(
                audioStrength = num(c.opt("audioStrength")) ?: 0.18,
                levelEaseRate = num(c.opt("levelEaseRate")) ?: 7.0,
                bandEaseRate = num(c.opt("bandEaseRate")) ?: 24.0,
                historyCount = num(c.opt("historyCount"))?.toInt(),
                historyHz = num(c.opt("historyHz")) ?: 12.0,
                interrupt = interrupt,
                mutedTint = num(c.opt("mutedTint")),
                mutedHue = num(c.opt("mutedHue")),
            )
            val v = VoiceOverrides(opts)
            val want = expected.getJSONArray(name)
            for (i in 0 until steps.length()) {
                val st = steps.getJSONObject(i)
                st.optJSONObject("metrics")?.let { m ->
                    val arr: JSONArray = m.getJSONArray("bands")
                    v.push(VoiceMetrics(m.getDouble("level"), List(arr.length()) { arr.getDouble(it) }))
                }
                if (st.has("state")) v.setState(AgentState.fromWire(st.getString("state"))!!)
                if (st.has("interrupt")) v.interrupt()
                if (st.has("historyCount")) v.setHistoryCount(st.getInt("historyCount"))
                if (st.has("muted")) v.muted = st.getBoolean("muted")
                if (st.has("reset")) v.reset()
                val got = v.overrides(st.getDouble("dt"))
                val w = want.getJSONObject(i)
                assertEquals("$name step $i keys", w.keySet(), got.keys)
                for (k in w.keySet()) assertEquals("$name step $i $k", w.getDouble(k), got.getValue(k), 1e-9)
            }
        }
    }

    @Test
    fun toneBurstsRiseAndDip() {
        // Same case as the iOS test: a 0.4 s gap dips but (as on Web) doesn't reach `listening`.
        val g = TestToneGenerator(48_000.0) { 0.5 }
        val spectrum = SpectrumAnalyser()
        val analysis = AudioAnalysis()
        val levels = List(60) {
            spectrum.push(g.next(1600))
            analysis.read(spectrum.byteFrequencyData()).level
        }
        val peak = levels.subList(0, 15).max()
        val gapMin = levels.subList(17, 29).min()
        assertTrue(peak > SPEAKING_LEVEL)
        assertTrue("peak $peak gap min $gapMin", gapMin < peak * 0.6)
        assertTrue(levels.subList(29, 45).max() > gapMin * 1.5)
    }

    @Test
    fun sampleRingDrainsOldestFirstAndKeepsNewest() {
        val ring = SampleRing(4)
        ring.write(floatArrayOf(1f, 2f, 3f, 4f, 5f, 6f))
        assertTrue(ring.drain().contentEquals(floatArrayOf(3f, 4f, 5f, 6f)))
        assertEquals(0, ring.drain().size)
    }
}
