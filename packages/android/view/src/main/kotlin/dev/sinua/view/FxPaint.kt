package dev.sinua.view

import android.graphics.BlurMaskFilter
import android.os.Build
import androidx.compose.ui.geometry.Offset
import androidx.compose.ui.graphics.BlendMode
import androidx.compose.ui.graphics.Brush
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.Path
import androidx.compose.ui.graphics.PathFillType
import androidx.compose.ui.graphics.ShaderBrush
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.TileMode
import androidx.compose.ui.graphics.asAndroidPath
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.drawIntoCanvas
import androidx.compose.ui.graphics.nativeCanvas
import androidx.compose.ui.graphics.toArgb
import uniffi.core_engine.ColorMode
import uniffi.core_engine.EffectRun
import uniffi.core_engine.Fill
import uniffi.core_engine.OrbFrame
import uniffi.core_engine.Polyline
import kotlin.math.hypot
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
 * then dots; plain fills; ink mirrors on dark themes. Shared by the live
 * [OrbCanvas] composable, the state thumbnails and [ImageExport]'s PNG
 * rasterizer via Compose's `CanvasDrawScope`, so none can drift from the
 * others -- the same role `drawFrame.ts` plays for the Web canvas + PNG
 * exporter.
 */
fun DrawScope.paintFxFrame(frame: OrbFrame, engineSize: Int, dark: Boolean, alphaScale: Float = 1f) {
    // Materials phase 1 (fills, effect runs): its own path, so frames without
    // them keep exactly the drawing below (raster parity).
    if (frame.fills.isNotEmpty() || frame.effects.isNotEmpty()) {
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
        if (fxHasHues(p)) {
            strokeHues(p, scale, mirror, p.a * k, 0)
            continue
        }
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
// Contract: docs/engine.md "Paint contract: fills and effects". Additive =
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

/**
 * The grain tile's value at (x, y), 0..1 (< 0.5 dark, >= 0.5 light; alpha = |v - 0.5| * 2): the same
 * integer hash as the Web's `grainValue` and FxPaint.swift (design note 22).
 */
fun fxGrainValue(x: Int, y: Int): Double {
    var h = (x * 73856093) xor (y * 19349663) xor 0x9E3779B9.toInt()
    h = (h xor (h ushr 13)) * 1274126177
    h = h xor (h ushr 16)
    return (h.toLong() and 0xFFFFFFFFL).toDouble() / 4294967296.0
}

/** The 64-px grain tile, built once; exporters embed it too ([fxGrainTile]). */
private val grainTile: android.graphics.Bitmap by lazy {
    val n = 64
    val px = IntArray(n * n)
    for (y in 0 until n) {
        for (x in 0 until n) {
            val v = fxGrainValue(x, y)
            val a = (kotlin.math.abs(v - 0.5) * 2 * 255).roundToInt()
            val c = if (v >= 0.5) 255 else 0
            px[y * n + x] = android.graphics.Color.argb(a, c, c, c)
        }
    }
    android.graphics.Bitmap.createBitmap(px, n, n, android.graphics.Bitmap.Config.ARGB_8888)
}

/** The 64-px grain tile (design note 22), for exporters that embed it. */
fun fxGrainTile(): android.graphics.Bitmap = grainTile

/**
 * An elliptical radial gradient (`kind` 2): a circle of radius rx (device px) whose local matrix
 * turns and squashes it onto the ellipse -- centre (x0, y0), first radius to (x1, y1), second `r`.
 */
private fun ellipticalShader(
    g: uniffi.core_engine.FillGradient,
    scale: Float,
    colors: IntArray,
    pos: FloatArray,
): android.graphics.Shader {
    val ux = g.x1 - g.x0
    val uy = g.y1 - g.y0
    val rx = max(1e-9, hypot(ux, uy))
    val cs = (ux / rx).toFloat()
    val sn = (uy / rx).toFloat()
    val k = max(1e-9, g.r / rx).toFloat()
    val shader = android.graphics.RadialGradient(
        0f,
        0f,
        maxOf(1e-3f, (rx * scale).toFloat()),
        colors,
        pos,
        android.graphics.Shader.TileMode.CLAMP,
    )
    val m = android.graphics.Matrix()
    m.setValues(floatArrayOf(cs, -sn * k, (g.x0 * scale).toFloat(), sn, cs * k, (g.y0 * scale).toFloat(), 0f, 0f, 1f))
    shader.setLocalMatrix(m)
    return shader
}

private fun DrawScope.paintWithMaterials(frame: OrbFrame, engineSize: Int, dark: Boolean, alphaScale: Float) {
    val mirror = dark && frame.colorMode != ColorMode.FIXED
    val k = alphaScale.toDouble()
    val scale = size.width / engineSize.toFloat()

    for (f in frame.fills) {
        if (f.points.size < 3) continue
        // Outer ring + every hole ring as one path; even-odd when there are holes.
        val path = Path()
        for (ring in listOf(f.points) + f.holes) {
            if (ring.size < 3) continue
            path.moveTo((ring[0].x * scale).toFloat(), (ring[0].y * scale).toFloat())
            for (i in 1 until ring.size) path.lineTo((ring[i].x * scale).toFloat(), (ring[i].y * scale).toFloat())
            path.close()
        }
        if (f.holes.isNotEmpty()) path.fillType = PathFillType.EvenOdd
        // Grain (blend 2, design note 22): the shape filled with the noise tile at `a`, one tile
        // pixel per dp however large the character is drawn, anchored at the origin.
        if (f.blend.toInt() == 2) {
            val shader = android.graphics.BitmapShader(
                grainTile,
                android.graphics.Shader.TileMode.REPEAT,
                android.graphics.Shader.TileMode.REPEAT,
            )
            shader.setLocalMatrix(android.graphics.Matrix().apply { setScale(density, density) })
            drawPath(path, ShaderBrush(shader), alpha = (f.a * k).coerceIn(0.0, 1.0).toFloat())
            continue
        }
        val g = f.gradient?.takeIf { it.stops.size >= 2 }
        val stops = g?.stops?.map { st ->
            st.offset.coerceIn(0.0, 1.0).toFloat() to
                fxInk(st.white, st.a * f.a * k, mirror, st.saturation, st.hue)
        }
        withEffect(
            f.blur,
            f.blend.toInt(),
            scale,
            framework = { paint ->
                if (g != null && stops != null) {
                    val colors = IntArray(stops.size) { stops[it].second.toArgb() }
                    val pos = FloatArray(stops.size) { stops[it].first }
                    paint.shader = if (g.kind.toInt() == 2) {
                        ellipticalShader(g, scale, colors, pos)
                    } else if (g.kind.toInt() == 1) {
                        android.graphics.RadialGradient(
                            (g.x0 * scale).toFloat(),
                            (g.y0 * scale).toFloat(),
                            maxOf(
                                1e-3f,
                                (
                                    g.r *
                                        scale
                                    ).toFloat(),
                            ),
                            colors,
                            pos,
                            android.graphics.Shader.TileMode.CLAMP,
                        )
                    } else {
                        android.graphics.LinearGradient(
                            (g.x0 * scale).toFloat(),
                            (g.y0 * scale).toFloat(),
                            (
                                g.x1 *
                                    scale
                                ).toFloat(),
                            (g.y1 * scale).toFloat(),
                            colors,
                            pos,
                            android.graphics.Shader.TileMode.CLAMP,
                        )
                    }
                } else {
                    paint.color = fxInk(f.white, f.a * k, mirror, f.saturation, f.hue).toArgb()
                }
                drawIntoCanvas { it.nativeCanvas.drawPath(path.asAndroidPath(), paint) }
            },
            compose = { mode ->
                if (g != null && stops != null) {
                    val brush = if (g.kind.toInt() == 2) {
                        ShaderBrush(
                            ellipticalShader(
                                g,
                                scale,
                                IntArray(stops.size) { stops[it].second.toArgb() },
                                FloatArray(stops.size) { stops[it].first },
                            ),
                        )
                    } else if (g.kind.toInt() == 1) {
                        Brush.radialGradient(
                            *stops.toTypedArray(),
                            center = Offset(
                                (g.x0 * scale).toFloat(),
                                (
                                    g.y0 *
                                        scale
                                    ).toFloat(),
                            ),
                            radius = maxOf(1e-3f, (g.r * scale).toFloat()),
                            tileMode = TileMode.Clamp,
                        )
                    } else {
                        Brush.linearGradient(
                            *stops.toTypedArray(),
                            start = Offset(
                                (g.x0 * scale).toFloat(),
                                (
                                    g.y0 *
                                        scale
                                    ).toFloat(),
                            ),
                            end = Offset((g.x1 * scale).toFloat(), (g.y1 * scale).toFloat()),
                            tileMode = TileMode.Clamp,
                        )
                    }
                    drawPath(path, brush, blendMode = mode)
                } else {
                    drawPath(path, fxInk(f.white, f.a * k, mirror, f.saturation, f.hue), blendMode = mode)
                }
            },
        )
    }

    val polyRuns = effectRuns(frame.effects, 2, frame.polylines.size)
    frame.polylines.forEachIndexed { i, p ->
        if (p.points.size < 2) return@forEachIndexed
        val e = polyRuns?.get(i)
        // A blurred per-vertex stroke keeps the one-path blur below with the
        // vertex-mean `hue` (see the per-vertex section): exact blur geometry.
        if (fxHasHues(p) && !((e?.blur ?: 0.0) > 0 && fxBlurSupported)) {
            strokeHues(p, scale, mirror, p.a * k, e?.blend?.toInt() ?: 0)
            return@forEachIndexed
        }
        val path = Path()
        path.moveTo((p.points[0].x * scale).toFloat(), (p.points[0].y * scale).toFloat())
        for (j in 1 until p.points.size) {
            path.lineTo(
                (p.points[j].x * scale).toFloat(),
                (p.points[j].y * scale).toFloat(),
            )
        }
        val color = fxInk(p.white, p.a * k, mirror, p.saturation, p.hue)
        val width = (p.w * scale).toFloat()
        withEffect(
            e?.blur ?: 0.0,
            e?.blend?.toInt() ?: 0,
            scale,
            framework = { paint ->
                paint.style = android.graphics.Paint.Style.STROKE
                paint.strokeWidth = width
                paint.strokeCap = android.graphics.Paint.Cap.ROUND
                paint.strokeJoin = android.graphics.Paint.Join.ROUND
                paint.color = color.toArgb()
                drawIntoCanvas { it.nativeCanvas.drawPath(path.asAndroidPath(), paint) }
            },
            compose = { mode ->
                drawPath(
                    path,
                    color,
                    style = Stroke(width = width, cap = StrokeCap.Round, join = StrokeJoin.Round),
                    blendMode = mode,
                )
            },
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

// ------------------------------------------- per-vertex stroke colour --
// Contract (docs/engine.md "Per-vertex stroke colour", agreed with families):
// with `hues`, every segment i-1 -> i is a round-capped stroke with a linear
// gradient between its vertices' inks at alpha 1 (a zero-length segment is a
// dot of diameter w in vertex i's colour), all in ONE layer (`saveLayer`)
// composited once at `a` with the element's blend -- so round-cap overlaps
// never double-paint.
// Blur is the exception. A framework Paint has no layer-level blur
// (RenderEffect needs a RenderNode / hardware canvas; the Studio's PNG export
// draws into a software bitmap), and blurring each segment inside the layer
// measured wrong: the overlapping soft fringes combine into a wider, stronger
// halo (mean 5/255 vs Web/iOS, docs/fx-view.md). So a *blurred* per-vertex
// polyline keeps the one-path BlurMaskFilter stroke with the vertex-mean `hue`:
// exact blur geometry, the hue sweep dropped -- measured within tolerance
// (mean <= 0.74), since the sweep isn't visible under a halo-sized blur.

/** Whether a polyline takes the per-vertex path (one hue per point). */
fun fxHasHues(p: Polyline): Boolean = p.hues.isNotEmpty() && p.hues.size == p.points.size

private fun DrawScope.strokeHues(p: Polyline, scale: Float, mirror: Boolean, alpha: Double, blend: Int) {
    val colors = IntArray(p.hues.size) { fxInk(p.white, 1.0, mirror, p.saturation, p.hues[it]).toArgb() }
    val w = (p.w * scale).toFloat()
    var x0 = Float.MAX_VALUE
    var y0 = Float.MAX_VALUE
    var x1 = -Float.MAX_VALUE
    var y1 = -Float.MAX_VALUE
    for (q in p.points) {
        val x = (q.x * scale).toFloat()
        val y = (q.y * scale).toFloat()
        x0 = minOf(x0, x)
        y0 = minOf(y0, y)
        x1 = maxOf(x1, x)
        y1 = maxOf(y1, y)
    }
    val pad = w / 2 + 2f
    val layer = android.graphics.Paint().apply {
        this.alpha = (alpha.coerceIn(0.0, 1.0) * 255).roundToInt()
        if (blend == 1) {
            if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.Q) {
                blendMode = android.graphics.BlendMode.PLUS
            } else {
                xfermode = android.graphics.PorterDuffXfermode(android.graphics.PorterDuff.Mode.ADD)
            }
        }
    }
    val seg = android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG).apply {
        style = android.graphics.Paint.Style.STROKE
        strokeWidth = w
        strokeCap = android.graphics.Paint.Cap.ROUND
    }
    val dot = android.graphics.Paint(android.graphics.Paint.ANTI_ALIAS_FLAG)
    drawIntoCanvas { canvas ->
        val nc = canvas.nativeCanvas
        val count = nc.saveLayer(x0 - pad, y0 - pad, x1 + pad, y1 + pad, layer)
        for (i in 1 until p.points.size) {
            val ax = (p.points[i - 1].x * scale).toFloat()
            val ay = (p.points[i - 1].y * scale).toFloat()
            val bx = (p.points[i].x * scale).toFloat()
            val by = (p.points[i].y * scale).toFloat()
            if (hypot(p.points[i].x - p.points[i - 1].x, p.points[i].y - p.points[i - 1].y) < 1e-9) {
                dot.color = colors[i]
                nc.drawCircle(bx, by, w / 2, dot)
                continue
            }
            seg.shader =
                android.graphics.LinearGradient(
                    ax,
                    ay,
                    bx,
                    by,
                    colors[i - 1],
                    colors[i],
                    android.graphics.Shader.TileMode.CLAMP,
                )
            nc.drawLine(ax, ay, bx, by, seg)
        }
        nc.restoreToCount(count)
    }
}
