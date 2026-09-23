package dev.sinua.view

import org.json.JSONObject
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File
import kotlin.math.abs
import kotlin.math.max

/**
 * Cross-language parity for the painter's colour conversion, against
 * `spec/paint-ink-vectors.json` (`packages/web/scripts/gen-ink-vectors.mjs`).
 *
 * This is the check that was missing. `OldPaint.kt` in `androidTest` compares
 * this painter against a frozen copy of its *own* past — useful, but silent on
 * whether Kotlin, Swift and the Web agree. Each converts differently: the Web
 * emits a CSS `hsla()` string and lets the **browser** convert (rounding
 * saturation and lightness to whole percent first), Swift does the arithmetic
 * by hand because SwiftUI's `Color` is HSB, and this file calls **Compose's**
 * `Color.hsl`.
 *
 * A JVM test, deliberately: `Color.hsl` is pure Kotlin arithmetic over a value
 * class, so this needs no emulator and runs in the leg CI already has.
 */
class InkVectorsTest {
    private fun vectors(): List<JSONObject> {
        // Walk up to the repo root: view/src/test/kotlin/dev/sinua/view -> repo
        var dir = File(System.getProperty("user.dir"))
        while (!File(dir, "spec/paint-ink-vectors.json").exists() && dir.parentFile != null) {
            dir = dir.parentFile
        }
        val file = File(dir, "spec/paint-ink-vectors.json")
        assertTrue("spec/paint-ink-vectors.json not found from ${System.getProperty("user.dir")}", file.exists())
        val arr = JSONObject(file.readText()).getJSONArray("cases")
        return (0 until arr.length()).map { arr.getJSONObject(it) }
    }

    @Test
    fun inkMatchesTheWebVectors() {
        val cases = vectors()
        assertTrue("the vector file looks truncated: ${cases.size}", cases.size > 500)

        var worst = 0.0
        var worstCase = ""
        var worstAlpha = 0.0
        var overOne = 0
        for (c in cases) {
            val color = fxInk(
                white = c.getDouble("white"),
                alpha = c.getDouble("alpha"),
                dark = c.getBoolean("dark"),
                saturation = c.getDouble("saturation"),
                hue = c.getDouble("hue"),
            )
            val rgba = c.getJSONArray("rgba")
            val d = max(
                abs(color.red * 255.0 - rgba.getDouble(0)),
                max(
                    abs(color.green * 255.0 - rgba.getDouble(1)),
                    abs(color.blue * 255.0 - rgba.getDouble(2)),
                ),
            )
            if (d > worst) {
                worst = d
                worstCase = c.toString()
            }
            if (d > 1) overOne += 1
            worstAlpha = max(worstAlpha, abs(color.alpha * 1.0 - rgba.getDouble(3)))
        }

        // The bound is measured, not guessed. The Web rounds saturation and
        // lightness to whole percent before the browser converts and this
        // painter does not, so a small systematic difference is expected and is
        // exactly what is worth pinning. 3/255 leaves room for an ordinary
        // rounding shift while staying nowhere near loose enough to hide a real
        // divergence -- a wrong hue sector or a swapped channel is tens of
        // units, not two.
        assertTrue("worst channel delta $worst at $worstCase", worst <= 3.0)
        assertTrue("alpha should pass through unchanged: $worstAlpha", worstAlpha <= 0.01)
        println("ink parity: ${cases.size} cases, worst $worst/255, $overOne case(s) over 1/255, worst alpha $worstAlpha")
    }
}
