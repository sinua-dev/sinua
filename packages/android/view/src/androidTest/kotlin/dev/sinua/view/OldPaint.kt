package dev.sinua.view

import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import uniffi.core_engine.ColorMode
import uniffi.core_engine.OrbFrame
import kotlin.math.max
import kotlin.math.min

// Snapshot of the Android Studio's OrbCanvas.kt's painter from before the move (2026-09-18) -- the parity baseline.

/**
 * Ink formula ported verbatim from the Web Studio's drawFrame.ts:
 * grayscale when [saturation] is 0 (every ported `orbs` mode), else HSL
 * with `white` as lightness -- Compose's [Color.hsl] is the same CSS
 * `hsl()` definition the Web renderer uses.
 */
fun oldInk(white: Double, alpha: Double, dark: Boolean, saturation: Double = 0.0, hue: Double = 0.0): Color {
    val w = min(1.0, max(0.0, white))
    val l = (if (dark) 1.0 - w else w).toFloat()
    val a = alpha.toFloat().coerceIn(0f, 1f)
    if (saturation <= 0.0) return Color(red = l, green = l, blue = l, alpha = a)
    val h = (((hue % 360.0) + 360.0) % 360.0).toFloat()
    return Color.hsl(hue = h, saturation = saturation.toFloat().coerceIn(0f, 1f), lightness = l, alpha = a)
}

/**
 * Paint contract (spec/orbs-spec.json, plus `Polyline`'s doc comment in
 * crates/core_engine/src/primitives.rs): polylines draw first, then lines,
 * then dots; plain fills; ink mirrors on dark themes. Shared by the live
 * [OrbCanvas] composable, the state thumbnails and [ImageExport]'s PNG
 * rasterizer via Compose's `CanvasDrawScope`, so none can drift from the
 * others -- the same role `drawFrame.ts` plays for the Web canvas + PNG
 * exporter.
 */
fun DrawScope.oldPaintFrame(frame: OrbFrame, engineSize: Int, dark: Boolean, alphaScale: Float = 1f) {
    // Colour mode (2026-09-18): "ink" frames mirror lightness on dark,
    // "fixed" frames keep `white` as-is in both themes.
    val mirror = dark && frame.colorMode != ColorMode.FIXED
    // `alphaScale` multiplies every draw's alpha (the Transitions cross-fade,
    // drawFrame.ts's per-draw `globalAlpha`); 1 leaves the contract untouched.
    val k = alphaScale.toDouble()
    val scale = size.width / engineSize.toFloat()
    // One stroked path per polyline, round caps + round joins -- never one
    // stroke per segment, which is the seam problem this primitive exists
    // to fix (see its Rust doc comment).
    for (p in frame.polylines) {
        if (p.points.size < 2) continue
        val path = Path()
        path.moveTo((p.points[0].x * scale).toFloat(), (p.points[0].y * scale).toFloat())
        for (i in 1 until p.points.size) {
            path.lineTo((p.points[i].x * scale).toFloat(), (p.points[i].y * scale).toFloat())
        }
        drawPath(
            path = path,
            color = oldInk(p.white, p.a * k, mirror, p.saturation, p.hue),
            style = Stroke(width = (p.w * scale).toFloat(), cap = StrokeCap.Round, join = StrokeJoin.Round),
        )
    }
    // `Line`s keep the upstream contract's default butt caps.
    for (l in frame.lines) {
        drawLine(
            color = oldInk(l.white, l.a * k, mirror, l.saturation, l.hue),
            start = Offset((l.x1 * scale).toFloat(), (l.y1 * scale).toFloat()),
            end = Offset((l.x2 * scale).toFloat(), (l.y2 * scale).toFloat()),
            strokeWidth = (l.w * scale).toFloat(),
        )
    }
    for (d in frame.dots) {
        drawCircle(
            color = oldInk(d.white, d.a * k, mirror, d.saturation, d.hue),
            radius = (d.r * scale).toFloat(),
            center = Offset((d.x * scale).toFloat(), (d.y * scale).toFloat()),
        )
    }
}
