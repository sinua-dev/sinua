// The bench's one formula (spec/bench/result.schema.json). Mirrors
// apps/fx-bench-web/src/metrics.ts and the Android Metrics.kt -- keep the
// three in sync.
import Foundation

struct Pct: Codable {
    var p50: Double, p95: Double, p99: Double, max: Double
}

/// Nearest-rank percentiles.
func pct(_ xs: [Double]) -> Pct {
    guard !xs.isEmpty else { return Pct(p50: 0, p95: 0, p99: 0, max: 0) }
    let s = xs.sorted()
    func at(_ p: Double) -> Double { s[min(s.count - 1, max(0, Int((p / 100 * Double(s.count)).rounded(.up)) - 1))] }
    return Pct(p50: r(at(50)), p95: r(at(95)), p99: r(at(99)), max: r(s[s.count - 1]))
}

func r(_ x: Double) -> Double { (x * 1000).rounded() / 1000 }

struct Summary {
    var targetFps: Double, frames: Int, durationS: Double, fps: Double
    var frameMs: Pct, computeMs: Pct, paintMs: Pct
    var droppedFrames: Int, hitchRatioMsPerS: Double
}

/// interval = 1000 / min(refreshHz, cap); late = max(0, round(dt / interval) - 1);
/// dropped = sum(late); hitch = sum(late) * interval; ratio = hitch / seconds.
func summarize(dts: [Double], computes: [Double], paints: [Double], durationS: Double, refreshHz: Double, cap: Double?) -> Summary {
    let target = min(refreshHz, cap ?? .infinity)
    let interval = 1000 / target
    var dropped = 0
    for dt in dts { dropped += max(0, Int((dt / interval).rounded()) - 1) }
    return Summary(
        targetFps: r(target), frames: dts.count, durationS: r(durationS), fps: r(Double(dts.count) / durationS),
        frameMs: pct(dts), computeMs: pct(computes), paintMs: pct(paints),
        droppedFrames: dropped, hitchRatioMsPerS: r(Double(dropped) * interval / durationS)
    )
}

/// Process CPU time (user + system), seconds.
func processCpuSeconds() -> Double {
    var u = rusage()
    getrusage(RUSAGE_SELF, &u)
    func s(_ t: timeval) -> Double { Double(t.tv_sec) + Double(t.tv_usec) / 1e6 }
    return s(u.ru_utime) + s(u.ru_stime)
}
