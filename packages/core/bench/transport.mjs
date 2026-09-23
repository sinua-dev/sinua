// Frame transport benchmark: JSON (frame_json -> JSON.parse) vs the packed
// Float64Array (frame_packed*), with and without rebuilding objects.
//   node bench/transport.mjs            (after `npm run build`)
// Prints median ms/frame per case. Not a test: numbers vary by machine;
// the LOG and docs/platforms/web.md record the reference run.
import { frameWithOverridesViaJson as frameWithOverrides, frameWithOverridesPacked, unpackFrame, readPacked } from "../dist/index.js";

const CASES = [
  ["working", {}],
  ["working", { glowStrength: 1 }],
  ["breathing", { glowStrength: 1 }],
  ["metering", {}],
  ["scrolling", {}],
];
const N = Number(process.env.BENCH_N ?? 400);

function median(xs) {
  const s = [...xs].sort((a, b) => a - b);
  return s[s.length >> 1];
}
function time(fn) {
  for (let i = 0; i < 40; i++) fn(i * 0.016); // warm-up
  const xs = [];
  for (let i = 0; i < N; i++) {
    const t0 = performance.now();
    fn(1 + i * 0.016);
    xs.push(performance.now() - t0);
  }
  return median(xs);
}

const rows = [];
for (const [state, ov] of CASES) {
  let sink = 0;
  const json = time((t) => { sink += frameWithOverrides(state, 64, t, ov).dots.length; });
  const packed = time((t) => { sink += frameWithOverridesPacked(state, 64, t, ov).dotCount; });
  const unpack = time((t) => { sink += unpackFrame(frameWithOverridesPacked(state, 64, t, ov)).dots.length; });
  const read = time((t) => {
    readPacked(frameWithOverridesPacked(state, 64, t, ov), { dot: (d, o) => { sink += d[o]; } });
  });
  const f = frameWithOverrides(state, 64, 1, ov);
  rows.push({
    case: `${state}${ov.glowStrength ? "+glow" : ""}`,
    elements: f.dots.length + f.lines.length + f.polylines.length,
    json: +json.toFixed(3),
    packed: +packed.toFixed(3),
    "packed+unpack": +unpack.toFixed(3),
    "packed+read": +read.toFixed(3),
    speedup: +(json / unpack).toFixed(1),
  });
  if (sink === -1) console.log(sink);
}
if (typeof process !== "undefined" && process.argv[2] === "--json") console.log(JSON.stringify(rows));
else console.table(rows);
