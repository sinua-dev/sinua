package dev.sinua.view

import android.graphics.BlurMaskFilter
import android.os.Build
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.asAndroidPath
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.graphics.toArgb
import uniffi.core_engine.ColorMode
import uniffi.core_engine.EffectRun
import uniffi.core_engine.OrbFrame
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

// The paint contract, moved verbatim from the Android Studio's OrbCanvas.kt
// (`ink` -> [fxInk], `paintFrame` -> [paintFxFrame]; the Studio's names now
// delegate here). Shared by SinuaView and the Studio's canvas, thumbnails,
// transitions and PNG export, so none can drift.

/**
 * Ink formula ported verbatim from the Web Studio's drawFrame.ts:
 * grayscale when [saturation] is 0 (every ported `orbs` mode), else HSL
 * with `white` as lightness -- Compose's [Color.hsl] is the same CSS
 * `hsl()` definition the Web renderer uses.
 */
fun fxInk(white: Double, alpha: Double, dark: Boolean, saturation: Double = 0.0, hue: Double = 0.0): Color {
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
 * then dots; ink mirrors on dark themes. Shared by the live
 * [OrbCanvas] composable, the state thumbnails and [ImageExport]'s PNG
 * rasterizer via Compose's `CanvasDrawScope`, so none can drift from the
 * others -- the same role `drawFrame.ts` plays for the Web canvas + PNG
 * exporter.
 */
fun DrawScope.paintFxFrame(frame: OrbFrame, engineSize: Int, dark: Boolean, alphaScale: Float = 1f) {
    // Effect runs (blur, blend, light): its own path, so frames without
    // them keep exactly the drawing below (raster parity).
    if (frame.effects.isNotEmpty()) {
        paintWithMaterials(frame, engineSize, dark, alphaScale)
        return
    }
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
            color = fxInk(p.white, p.a * k, mirror, p.saturation, p.hue),
            style = Stroke(width = (p.w * scale).toFloat(), cap = StrokeCap.Round, join = StrokeJoin.Round),
        )
    }
    // `Line`s keep the upstream contract's default butt caps.
    for (l in frame.lines) {
        drawLine(
            color = fxInk(l.white, l.a * k, mirror, l.saturation, l.hue),
            start = Offset((l.x1 * scale).toFloat(), (l.y1 * scale).toFloat()),
            end = Offset((l.x2 * scale).toFloat(), (l.y2 * scale).toFloat()),
            strokeWidth = (l.w * scale).toFloat(),
        )
    }
    for (d in frame.dots) {
        drawCircle(
            color = fxInk(d.white, d.a * k, mirror, d.saturation, d.hue),
            radius = (d.r * scale).toFloat(),
            center = Offset((d.x * scale).toFloat(), (d.y * scale).toFloat()),
        )
    }
}

// ------------------------------------------------ materials phase 1 --
// Contract: docs/engine.md "Paint contract: effects". Additive =
// BlendMode.Plus (every API level). Blur = BlurMaskFilter on the framework
// Paint, reliable with hardware acceleration from API 29; below that,
// elements are drawn unblurred (documented fallback). Skia converts a mask
// radius to sigma as 0.57735 * radius + 0.5 (SkBlurMask.cpp), inverted here.

/** Whether per-element blur is drawn (BlurMaskFilter under HW acceleration: API 29+). */
val fxBlurSupported: Boolean get() = Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q

internal fun blurMaskRadius(sigmaPx: Double): Float = (((sigmaPx - 0.5) / 0.57735).coerceAtLeast(0.1)).toFloat()

private fun effectRuns(effects: List<EffectRun>, target: Int, count: Int): Array<EffectRun?>? {
    var map: Array<EffectRun?>? = null
    for (e in effects) {
        if (e.target.toInt() != target || e.count.toInt() <= 0) continue
        val m = map ?: arrayOfNulls<EffectRun>(count).also { map = it }
        val end = minOf(count, e.start.toInt() + e.count.toInt())
        for (i in e.start.toInt() until end) m[i] = e
    }
    return map
}

/**
 * Draw one element with blur sigma (engine units) and blend. With blur on a
 * supporting device the element goes through a framework Paint carrying the
 * mask filter; otherwise Compose draws it (additive via BlendMode.Plus).
 */
private fun DrawScope.withEffect(
    blurSigma: Double,
    blend: Int,
    scale: Float,
    framework: (android.graphics.Paint) -> Unit,
    compose: (BlendMode) -> Unit,
) {
    val mode = if (blend == 1) BlendMode.Plus else BlendMode.SrcOver
    if (blurSigma > 0 && fxBlurSupported) {
        val paint = android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG)
        paint.maskFilter = BlurMaskFilter(blurMaskRadius(blurSigma * scale), BlurMaskFilter.Blur.NORMAL)
        if (blend == 1 &&
            Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q
        ) {
            paint.blendMode = android.graphics.BlendMode.PLUS
        }
        framework(paint)
    } else {
        compose(mode)
    }
}

private fun DrawScope.paintWithMaterials(frame: OrbFrame, engineSize: Int, dark: Boolean, alphaScale: Float) {
    val mirror = dark && frame.colorMode != ColorMode.FIXED
    val k = alphaScale.toDouble()
    val scale = size.width / engineSize.toFloat()

    for (p in frame.polylines) {
        if (p.points.size < 2) continue
        val path = Path()
        path.moveTo((p.points[0].x * scale).toFloat(), (p.points[0].y * scale).toFloat())
        for (j in 1 until p.points.size) {
            path.lineTo((p.points[j].x * scale).toFloat(), (p.points[j].y * scale).toFloat())
        }
        drawPath(
            path = path,
            color = fxInk(p.white, p.a * k, mirror, p.saturation, p.hue),
            style = Stroke(width = (p.w * scale).toFloat(), cap = StrokeCap.Round, join = StrokeJoin.Round),
        )
    }
    val lineRuns = effectRuns(frame.effects, 1, frame.lines.size)
    frame.lines.forEachIndexed { i, l ->
        val e = lineRuns?.get(i)
        val color = fxInk(l.white, l.a * k, mirror, l.saturation, l.hue)
        val a = Offset((l.x1 * scale).toFloat(), (l.y1 * scale).toFloat())
        val b = Offset((l.x2 * scale).toFloat(), (l.y2 * scale).toFloat())
        val width = (l.w * scale).toFloat()
        withEffect(
            e?.blur ?: 0.0,
            e?.blend?.toInt() ?: 0,
            scale,
            framework = { paint ->
                paint.style = android.graphics.Paint.Style.STROKE
                paint.strokeWidth = width
                paint.color = color.toArgb()
                drawIntoCanvas { it.nativeCanvas.drawLine(a.x, a.y, b.x, b.y, paint) }
            },
            compose = { mode -> drawLine(color, a, b, strokeWidth = width, blendMode = mode) },
        )
    }
    val dotRuns = effectRuns(frame.effects, 0, frame.dots.size)
    frame.dots.forEachIndexed { i, d ->
        val e = dotRuns?.get(i)
        if (e?.blend?.toInt() == FX_LIGHT) {
            drawLight(d, scale, dark, mirror, k)
            return@forEachIndexed
        }
        val color = fxInk(d.white, d.a * k, mirror, d.saturation, d.hue)
        val c = Offset((d.x * scale).toFloat(), (d.y * scale).toFloat())
        val r = (d.r * scale).toFloat()
        withEffect(
            e?.blur ?: 0.0,
            e?.blend?.toInt() ?: 0,
            scale,
            framework = { paint ->
                paint.color = color.toArgb()
                drawIntoCanvas { it.nativeCanvas.drawCircle(c.x, c.y, r, paint) }
            },
            compose = { mode -> drawCircle(color, radius = r, center = c, blendMode = mode) },
        )
    }
}

// ---------------------------------------------------------------- light --

/**
 * `blend` 3 on dots (1.15 light volumes, docs/engine.md *Light*): each dot is a soft radial
 * glow of radius `r` -- a bright core, its colour, then nothing -- added up on a dark
 * background (`BlendMode.Plus`). A light background can't take added light, so there the
 * glow, its core no brighter than the colour, is laid on normally. The Web's `drawLight`.
 */
internal const val FX_LIGHT = 3

private fun DrawScope.drawLight(d: uniffi.core_engine.Dot, scale: Float, dark: Boolean, mirror: Boolean, k: Double) {
    val r = (d.r * scale).toFloat()
    if (r <= 0f) return
    val l = if (mirror) 1.0 - d.white else d.white
    val sprite = lightSprite(d.hue, d.saturation, l, dark)
    val x = (d.x * scale).toFloat()
    val y = (d.y * scale).toFloat()
    val paint = if (dark) lightPaintAdd else lightPaintOver
    paint.alpha = ((d.a * k).coerceIn(0.0, 1.0) * 255).roundToInt()
    lightRect.set(x - r, y - r, x + r, y + r)
    drawIntoCanvas { it.nativeCanvas.drawBitmap(sprite, null, lightRect, paint) }
}

// Lazy: created on the first light dot, so loading this file (JVM unit tests) needs no Paint.
private val lightRect by lazy { android.graphics.RectF() }
private val lightPaintAdd by lazy {
    android.graphics.Paint(android.graphics.Paint.FILTER_BITMAP_FLAG).apply {
        xfermode = android.graphics.PorterDuffXfermode(android.graphics.PorterDuff.Mode.ADD)
    }
}
private val lightPaintOver by lazy { android.graphics.Paint(android.graphics.Paint.FILTER_BITMAP_FLAG) }

/** One 32-px glow per colour (hue, saturation and lightness in whole steps), drawn once and reused. */
private val lightSprites = object : java.util.LinkedHashMap<Long, android.graphics.Bitmap>(64, 0.75f, true) {
    override fun removeEldestEntry(eldest: MutableMap.MutableEntry<Long, android.graphics.Bitmap>?) = size > 512
}

private fun lightSprite(hue: Double, sat: Double, light: Double, dark: Boolean): android.graphics.Bitmap {
    val h = hue.roundToInt()
    val s = (sat * 100).roundToInt()
    val l = (light * 100).roundToInt()
    val key = (h.toLong() shl 24) or (s.toLong() shl 16) or (l.toLong() shl 8) or (if (dark) 1L else 0L)
    lightSprites[key]?.let { return it }
    val core = if (dark) min(0.97, l / 100.0 + 0.30) else max(0.10, l / 100.0 - 0.22)
    val tone = if (dark) l / 100.0 else max(0.10, l / 100.0 - 0.22)
    val ink = { w: Double, a: Double -> fxInk(w, a, false, s / 100.0, h.toDouble()).toArgb() }
    val size = 32
    val bmp = android.graphics.Bitmap.createBitmap(size, size, android.graphics.Bitmap.Config.ARGB_8888)
    val paint = android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG)
    paint.shader = android.graphics.RadialGradient(
        size / 2f,
        size / 2f,
        size / 2f,
        intArrayOf(ink(core, 1.0), ink(tone, 0.9), ink(tone, 0.22), ink(tone, 0.0)),
        floatArrayOf(0f, 0.18f, 0.45f, 1f),
        android.graphics.Shader.TileMode.CLAMP,
    )
    android.graphics.Canvas(bmp).drawRect(0f, 0f, size.toFloat(), size.toFloat(), paint)
    lightSprites[key] = bmp
    return bmp
}
