package dev.sinua.core

import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.core_engine.OrbFrame
import uniffi.core_engine.effectInfo
import uniffi.core_engine.frameWithOverrides

/**
 * One-shot effects through the Android bindings: spec/effect-vectors.json (the file Web and
 * iOS read too), summarised the same way. docs/fx-view.md, *One-shot effects*.
 */
@RunWith(AndroidJUnit4::class)
class EffectVectorsTests {
    private val vectors: JSONObject by lazy {
        val ctx = InstrumentationRegistry.getInstrumentation().context
        JSONObject(ctx.assets.open("effect-vectors.json").bufferedReader().readText())
    }

    private fun summary(f: OrbFrame): Map<String, Double> {
        var alpha = 0.0
        var hue = 0.0
        var nh = 0
        var sx = 0.0
        var n = 0
        for (d in f.dots) {
            alpha += d.a
            if (d.saturation > 0) {
                hue += d.hue
                nh++
            }
            sx += d.x
            n++
        }
        for (l in f.lines) {
            alpha += l.a
            if (l.saturation > 0) {
                hue += l.hue
                nh++
            }
            sx += l.x1 + l.x2
            n += 2
        }
        for (p in f.polylines) {
            alpha += p.a
            if (p.saturation > 0) {
                hue += p.hue
                nh++
            }
            for (q in p.points) {
                sx += q.x
                n++
            }
        }
        return mapOf(
            "dots" to f.dots.size.toDouble(),
            "lines" to f.lines.size.toDouble(),
            "polylines" to f.polylines.size.toDouble(),
            "alpha" to alpha,
            "hue" to if (nh == 0) 0.0 else hue / nh,
            "cx" to if (n == 0) 0.0 else sx / n,
        )
    }

    @Test
    fun eachEffectRendersAsTheEngineVectorsSay() {
        val size = vectors.getInt("size").toUInt()
        val t = vectors.getDouble("t")
        val cases = vectors.getJSONArray("cases")
        assertTrue(cases.length() >= 72)
        for (i in 0 until cases.length()) {
            val c = cases.getJSONObject(i)
            val info = effectInfo(c.getString("effect"))!!
            val reduced = c.getBoolean("reduced")
            val f = frameWithOverrides(
                c.getString("pattern"),
                size,
                t,
                mapOf(
                    "effectCode" to info.code.toDouble(),
                    "effectAge" to c.getDouble("age"),
                    "effectReduced" to if (reduced) 1.0 else 0.0,
                ),
            )!!
            val got = summary(f)
            val want = c.getJSONObject("summary")
            for (k in listOf("dots", "lines", "polylines", "alpha", "hue", "cx")) {
                assertEquals("${c.getString("pattern")} ${c.getString("effect")} ${c.getDouble("age")} $reduced $k", want.getDouble(k), got[k]!!, 1e-5)
            }
        }
        assertNull(effectInfo("confetti"))
    }
}
