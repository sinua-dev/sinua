// The large-size transition check (design note 40, V9): the lattice trio through their
// voice-state changes, rasterized in colour at 512 px. The contract suite (check.mjs) rasters
// ink at 96 px, which a colour field's zoom never shows; at video size it read as a jump
// (glowing's thinking zoom, 2.5x in under a second). Each change's worst frame is compared
// with the pattern's liveliest steady motion (any of its voice states, as DevinFit compares
// with the orb's natural speaking motion); exits 1 if one is over the bound.
//   node scripts/transitions/large.mjs [--json out.json]
import { writeFileSync } from "node:fs";
import { core, side, SIZE, DT } from "./drive.mjs";

const N = 512;
const PATTERNS = ["glowing", "calibrating", "progressing"];
const PAIRS = [["listening", "thinking"], ["thinking", "speaking"], ["speaking", "listening"], ["idle", "listening"]];
// Measured with glowing's thinking zoom at 3.0: 2.75 / 4.04; at 1.6 (design note 40): <= 1.36.
const BOUND = 1.6;
// Changes meant to be big, each with its measured value (a regression still shows).
const EXCEPT = {
  "progressing idle>listening": { bound: 3.0, why: "waking up: ink 0.72 -> 1 and the pulse stops in 0.3 s (measured 2.69); a candidate to soften" },
};

const rgbOf = (d) => {
  // hue/saturation at mid lightness, mixed toward white by `white`.
  const h = (((d.hue ?? 0) % 360) + 360) % 360 / 60, s = d.saturation ?? 0, c = s, x = c * (1 - Math.abs((h % 2) - 1));
  const [r, g, b] = h < 1 ? [c, x, 0] : h < 2 ? [x, c, 0] : h < 3 ? [0, c, x] : h < 4 ? [0, x, c] : h < 5 ? [x, 0, c] : [c, 0, x];
  const m = 0.5 - c / 2, w = d.white ?? 0;
  return [r + m, g + m, b + m].map((v) => v * (1 - w) + w);
};

/** Colour coverage of a frame's dots on an N×N grid (3 channels), weighted by `k`. */
function raster(fr, out, k) {
  if (!fr) return;
  const s = N / SIZE;
  for (const d of fr.dots) {
    const cx = d.x * s, cy = d.y * s, r = Math.max(0.5, d.r * s), a = d.a * k, col = rgbOf(d), r2 = r * r;
    for (let y = Math.max(0, Math.floor(cy - r)); y <= Math.min(N - 1, cy + r); y++)
      for (let x = Math.max(0, Math.floor(cx - r)); x <= Math.min(N - 1, cx + r); x++) {
        const dx = x + 0.5 - cx, dy = y + 0.5 - cy;
        if (dx * dx + dy * dy > r2 + 0.5) continue;
        const i = (y * N + x) * 3;
        out[i] += a * col[0]; out[i + 1] += a * col[1]; out[i + 2] += a * col[2];
      }
  }
}
const diff = (a, b) => {
  let t = 0;
  for (let i = 0; i < a.length; i++) t += Math.abs(Math.min(1, a[i]) - Math.min(1, b[i]));
  return (t / a.length) * 1000;
};

/** The worst frame of `state` held for 2 s (per mille). */
function steady(p, state, t0 = 10) {
  let phase = t0 * side(p, state).speed, prev = null, max = 0;
  for (let i = 0; i * DT <= 2; i++) {
    phase += DT * side(p, state).speed;
    const g = new Float32Array(N * N * 3);
    raster(core.frameWithOverrides(p, SIZE, phase, side(p, state).overrides), g, 1);
    if (prev) max = Math.max(max, diff(prev, g));
    prev = g;
  }
  return max;
}

/** One change from `a` to `b` at 2 s: its worst frame in the second after (per mille). */
function measure(p, a, b, t0 = 10) {
  const tr = new core.StateTransition();
  let cur = a, phase = t0 * side(p, a).speed, prev = null, trMax = 0;
  for (let i = 0; i * DT <= 4; i++) {
    const t = i * DT;
    if (cur === a && t >= 2) {
      const pair = core.fxSpecTransition("{}", a, b);
      tr.start(pair.duration, pair.curve);
      cur = b;
    }
    if (i > 0) tr.advance(DT);
    const to = side(p, cur);
    phase += DT * tr.speed(to, SIZE);
    const f = tr.frames(to, SIZE, phase, {});
    const g = new Float32Array(N * N * 3);
    raster(f.frame, g, f.blend);
    if (f.previous) raster(f.previous, g, 1 - f.blend);
    if (prev) {
      const d = diff(prev, g);
      if (t >= 2 && t < 3) trMax = Math.max(trMax, d);
    }
    prev = g;
  }
  return trMax;
}

const failures = [], report = [];
for (const p of PATTERNS) {
  const row = [];
  const st = Math.max(...["idle", "listening", "thinking", "speaking"].map((s) => steady(p, s)));
  for (const [a, b] of PAIRS) {
    const tr = measure(p, a, b);
    const ratio = tr / Math.max(1e-3, st);
    report.push({ pattern: p, pair: `${a}>${b}`, transition: tr, steady: st, ratio });
    const ex = EXCEPT[`${p} ${a}>${b}`];
    row.push(`${a.slice(0, 4)}>${b.slice(0, 4)} ${ratio.toFixed(2)}${ex ? "*" : ""}`);
    if (ratio > (ex?.bound ?? BOUND)) failures.push(`${p} ${a}>${b}: ${ratio.toFixed(2)}x its steady motion > ${ex?.bound ?? BOUND}`);
  }
  console.log(`${p.padEnd(12)} ${row.join("  ")}`);
}
const out = process.argv.includes("--json") ? process.argv[process.argv.indexOf("--json") + 1] : null;
if (out) writeFileSync(out, JSON.stringify(report, null, 1));
if (failures.length) {
  console.error(`\ntransitions (large): ${failures.length} change(s) jump at 512 px:\n  ${failures.join("\n  ")}`);
  process.exit(1);
}
for (const [k, v] of Object.entries(EXCEPT)) console.log(`* ${k}: ${v.why}`);
console.log("\ntransitions (large): every lattice change within its steady motion at 512 px");
