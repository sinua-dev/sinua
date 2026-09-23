// The bench's one formula (spec/bench/result.schema.json). Mirrors
// apps/fx-bench-web/src/metrics.ts and the iOS Metrics.swift -- keep the
// three in sync.
package dev.sinua.fxbench

import kotlin.math.ceil
import kotlin.math.max
import kotlin.math.min
import kotlin.math.roundToInt

data class Pct(val p50: Double, val p95: Double, val p99: Double, val max: Double)

fun r(x: Double): Double = (x * 1000).roundToInt() / 1000.0

/** Nearest-rank percentiles. */
fun pct(xs: List<Double>): Pct {
    if (xs.isEmpty()) return Pct(0.0, 0.0, 0.0, 0.0)
    val s = xs.sorted()
    fun at(p: Double) = s[min(s.size - 1, max(0, ceil(p / 100 * s.size).toInt() - 1))]
    return Pct(r(at(50.0)), r(at(95.0)), r(at(99.0)), r(s.last()))
}

data class Summary(
    val targetFps: Double, val frames: Int, val durationS: Double, val fps: Double,
    val frameMs: Pct, val computeMs: Pct, val paintMs: Pct,
    val droppedFrames: Int, val hitchRatioMsPerS: Double,
)

/**
 * interval = 1000 / min(refreshHz, cap); late = max(0, round(dt / interval) - 1);
 * dropped = sum(late); hitch = sum(late) * interval; ratio = hitch / seconds.
 */
fun summarize(dts: List<Double>, computes: List<Double>, paints: List<Double>, durationS: Double, refreshHz: Double, cap: Double?): Summary {
    val target = min(refreshHz, cap ?: Double.POSITIVE_INFINITY)
    val interval = 1000 / target
    var dropped = 0
    for (dt in dts) dropped += max(0, (dt / interval).roundToInt() - 1)
    return Summary(
        r(target), dts.size, r(durationS), r(dts.size / durationS),
        pct(dts), pct(computes), pct(paints),
        dropped, r(dropped * interval / durationS),
    )
}
