package dev.sinua.view

import androidx.compose.ui.geometry.Size
import androidx.compose.ui.graphics.Canvas
import androidx.compose.ui.graphics.ImageBitmap
import androidx.compose.ui.graphics.asAndroidBitmap
import androidx.compose.ui.graphics.drawscope.CanvasDrawScope
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.graphics.toArgb
import androidx.compose.ui.unit.Density
import androidx.compose.ui.unit.LayoutDirection
import androidx.test.ext.junit.runners.AndroidJUnit4
import org.junit.Assert.assertTrue
import org.junit.Test
import org.junit.runner.RunWith
import uniffi.core_engine.ColorMode
import uniffi.core_engine.OrbFrame
import uniffi.core_engine.Point
import uniffi.core_engine.Polyline
import kotlin.math.abs
import kotlin.math.max

/**
 * Seam check for per-vertex stroke colour (docs/engine.md), the same as iOS
 * PerVertexSeamTests: constant `hues` through the per-vertex path must look like
 * the one-stroke reference; a naive per-segment composite (each segment straight
 * at `a`) is the negative control whose cap overlaps double-paint.
 */
@RunWith(AndroidJUnit4::class)
class PerVertexSeamTest {
    private fun polyline(hues: List<Double>) = Polyline(
        points = listOf(
            8.0 to 40.0,
            16.0 to 12.0,
            24.0 to 44.0,
            30.0 to 20.0,
            30.0 to 20.0,
            40.0 to 48.0,
            48.0 to 10.0,
            56.0 to 36.0,
        )
            .map { Point(it.first, it.second) },
        white = 0.5,
        a = 0.5,
        w = 3.0,
        saturation = 1.0,
        hue = 200.0,
        hues = hues,
    )

    private fun frame(p: Polyline) = OrbFrame(listOf(), listOf(), listOf(p), ColorMode.INK, listOf(), listOf())

    /** 256x256 over white (the tolerance metric's convention), as ARGB ints. */
    private fun render(draw: DrawScope.() -> Unit): IntArray {
        val img = ImageBitmap(256, 256)
        CanvasDrawScope().draw(Density(1f), LayoutDirection.Ltr, Canvas(img), Size(256f, 256f)) {
            drawRect(androidx.compose.ui.graphics.Color.White)
            draw()
        }
        val out = IntArray(256 * 256)
        img.asAndroidBitmap().getPixels(out, 0, 256, 0, 0, 256, 256)
        return out
    }

    private data class Stats(val mean: Double, val p99: Double, val max: Double, val over24: Int)

    private fun stats(a: IntArray, b: IntArray): Stats {
        val d = DoubleArray(a.size * 3)
        var over = 0
        for (i in a.indices) {
            var px = 0.0
            for (c in 0 until 3) {
                val sh = 16 - 8 * c
                val v = abs(((a[i] shr sh) and 0xFF) - ((b[i] shr sh) and 0xFF)).toDouble()
                d[3 * i + c] = v
                px = max(px, v)
            }
            if (px > 24) over++
        }
        d.sort()
        return Stats(d.average(), d[(d.size * 0.99).toInt()], d.last(), over)
    }

    @Test
    fun constantHuesMatchOneStrokeAndNaiveSegmentsDoNot() {
        val one = frame(polyline(emptyList()))
        val layered = frame(polyline(List(8) { 200.0 }))
        assertTrue(fxHasHues(layered.polylines[0]))
        val ref = render { paintFxFrame(one, 64, false) }
        val got = render { paintFxFrame(layered, 64, false) }
        val naive = render {
            val p = layered.polylines[0]
            val paint = android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG).apply {
                style = android.graphics.Paint.Style.STROKE
                strokeWidth = (p.w * 4).toFloat()
                strokeCap = android.graphics.Paint.Cap.ROUND
                color = fxInk(p.white, p.a, false, p.saturation, 200.0).toArgb()
            }
            drawIntoCanvas { c ->
                for (i in 1 until p.points.size) {
                    c.nativeCanvas.drawLine(
                        (p.points[i - 1].x * 4).toFloat(),
                        (p.points[i - 1].y * 4).toFloat(),
                        (p.points[i].x * 4).toFloat(),
                        (p.points[i].y * 4).toFloat(),
                        paint,
                    )
                }
            }
        }
        val l = stats(ref, got)
        val n = stats(ref, naive)
        android.util.Log.i("FxSeam", "layered vs one-stroke: $l")
        android.util.Log.i("FxSeam", "naive   vs one-stroke: $n")
        assertTrue("layered mean ${l.mean}", l.mean <= 2)
        assertTrue("layered p99 ${l.p99}", l.p99 <= 24)
        assertTrue("the negative control must show the double-painted joints (${n.max})", n.max > 40)
        // What remains is AA on the outline where segment edges overlap inside the layer (see iOS).
        assertTrue("no seam: ${l.over24} px vs naive ${n.over24}", l.over24 * 10 < n.over24)
    }
}
