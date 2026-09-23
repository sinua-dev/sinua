// The bench's one formula (spec/bench/result.schema.json; mirrored in the
// iOS and Android bench apps' Metrics.swift / Metrics.kt -- keep the three
// in sync). Inputs are SinuaView's onFrame stats for the measured window.

export interface Pct { p50: number; p95: number; p99: number; max: number }

/** Nearest-rank percentiles. */
export function pct(xs: number[]): Pct {
  if (xs.length === 0) return { p50: 0, p95: 0, p99: 0, max: 0 };
  const s = [...xs].sort((a, b) => a - b);
  const at = (p: number) => s[Math.min(s.length - 1, Math.max(0, Math.ceil((p / 100) * s.length) - 1))];
  return { p50: r(at(50)), p95: r(at(95)), p99: r(at(99)), max: r(s[s.length - 1]) };
}

const r = (x: number) => Math.round(x * 1000) / 1000;

export interface Summary {
  targetFps: number;
  frames: number;
  durationS: number;
  fps: number;
  frameMs: Pct;
  computeMs: Pct;
  paintMs: Pct;
  droppedFrames: number;
  hitchRatioMsPerS: number;
}

/**
 * interval = 1000 / min(refreshHz, cap); late = max(0, round(dt / interval) - 1)
 * (whole display intervals a frame missed); dropped = sum(late);
 * hitch = sum(late) * interval; ratio = hitch / seconds (Apple's hitch time
 * ratio, ms per s). Vsync-quantized on purpose: summing raw `dt - interval`
 * counted ordinary rAF timestamp jitter (17.3 vs 16.7 ms) as 3-10 ms/s of
 * "hitch" with no frame actually late (first desktop run, 2026-09-19).
 */
export function summarize(dts: number[], computes: number[], paints: number[], durationS: number, refreshHz: number, cap: number | null): Summary {
  const targetFps = Math.min(refreshHz, cap ?? Infinity);
  const interval = 1000 / targetFps;
  let dropped = 0;
  let hitch = 0;
  for (const dt of dts) {
    const late = Math.max(0, Math.round(dt / interval) - 1);
    dropped += late;
    hitch += late * interval;
  }
  return {
    targetFps: r(targetFps),
    frames: dts.length,
    durationS: r(durationS),
    fps: r(dts.length / durationS),
    frameMs: pct(dts),
    computeMs: pct(computes),
    paintMs: pct(paints),
    droppedFrames: dropped,
    hitchRatioMsPerS: r(hitch / durationS),
  };
}
