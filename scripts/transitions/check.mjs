// The transition contract's bounds (design note 31, TS0/TS4/TS5), run in CI after the
// packages build: every pattern through every voice-state pair at 10 s and 300 s plus an
// interruption, and realistic conversations through the GPT-Live session core. Exits 1 if
// any bound is crossed. `--quick`: a few patterns only (while iterating).
//   node scripts/transitions/check.mjs [--quick] [--json out.json]
import { writeFileSync } from "node:fs";
import { run, score, ORBS, CHARACTERS } from "./drive.mjs";
import { SCENARIOS, states } from "./scenarios.mjs";

const args = process.argv.slice(2);
const quick = args.includes("--quick");
const jsonOut = args.includes("--json") ? args[args.indexOf("--json") + 1] : null;

// 1 = a perfect transition. The general bound has a little headroom over what the
// contract measures (design note 31); the exceptions are motion that is meant to travel.
const GENERAL = { peak: 2.5, path: 2.6 };
const RING = { path: 3.2, why: "thinking starts the ring's own brightness pulse with the change" };
const STEPS = (what) => ({ peak: Infinity, why: `${what} steps: single-frame changes are its steady motion too` });
const EXCEPT = {
  speaking: { peak: 3, path: 5.5, why: "the spectrum's bars join one at a time and slide (the user's pick: the join chain)" },
  concluding: { path: 4.5, why: "crystallize assembles and scatters on its own cycle; a burst can fall in the window" },
  muted: { peak: 6.5, path: 5.5, why: "a rigid turn of a dense lattice (yaw): every dot moves a little every frame" },
  calibrating: { peak: 12, why: "chladni snaps to a new mode every holdDuration (a smoothing is the next step)" },
  beep: { peak: 3.6, path: 3.2, why: "the arms swing out wide, a big continuous move" },
  completing: RING,
  tracking: RING,
  stepping: RING,
  measuring: RING,
  signaling: STEPS("LED"),
  metering: STEPS("meter"),
  playing: { ...STEPS("playback"), path: 2.6 },
};
const bound = (p, k) => EXCEPT[p]?.[k] ?? GENERAL[k];

const V = ["idle", "listening", "thinking", "speaking"];
const pairs = [["initializing", "idle"]];
for (const a of V) for (const b of V) if (a !== b) pairs.push([a, b]);
const patterns = quick ? ["glowing", "tracking", "speaking", "buzzy"] : [...ORBS, ...CHARACTERS];

const failures = [];
const report = { pairs: [], scenarios: [] };
for (const p of patterns) {
  let worst = { peak: 0, path: 0 };
  for (const t0 of [10, 300]) {
    const runs = pairs.map(([a, b]) => [`${a}>${b}`, [[0, a], [2, b]], 5]);
    runs.push(["idle>speaking>!listening", [[0, "idle"], [2, "speaking"], [2.3, "listening"]], 5.3]);
    for (const [name, events, total] of runs) {
      const s = score(run(p, events, { t0, total }));
      report.pairs.push({ pattern: p, t0, pair: name, ...s });
      for (const k of ["peak", "path"]) {
        worst[k] = Math.max(worst[k], s[k]);
        if (s[k] > bound(p, k)) failures.push(`${p} ${name} at ${t0} s: ${k} ${s[k].toFixed(2)} > ${bound(p, k)}`);
      }
    }
  }
  console.log(`${p.padEnd(14)} peak ${worst.peak.toFixed(2).padStart(5)}  path ${worst.path.toFixed(2).padStart(5)}${EXCEPT[p] ? `   (${EXCEPT[p].why})` : ""}`);
}

// Conversations: each answer is one speaking stretch (the 1 s tail holds pauses under it),
// and the transitions stay inside the bounds on representative patterns, as smooth at the
// end of a 5-minute session as at its start.
const SAMPLE = quick ? ["glowing"] : ["glowing", "working", "breathing", "tracking", "speaking", "buzzy"];
for (const sc of SCENARIOS) {
  const { changes, answers, seconds } = states(sc);
  const entries = changes.filter((c) => c[1] === "speaking").length;
  // A pause longer than the 1 s tail ends speaking once (the 1500 ms pause).
  const longPauses = sc.segments.flatMap((s) => (Array.isArray(s.pauses) ? s.pauses : [])).filter(([, ms]) => ms > 1000).length * (sc.repeat ?? 1);
  if (entries > answers + longPauses) failures.push(`${sc.name}: ${entries} speaking stretches for ${answers} answers (flapping)`);
  const row = { name: sc.name, answers, speakingStretches: entries, patterns: {} };
  for (const p of SAMPLE) {
    if (sc.window) {
      const [a0, a1, b0, b1] = sc.window;
      const part = (from, to) => score(run(p, rebase(changes, from), { t0: 10 + from, total: to - from }));
      const start = part(a0, a1);
      const end = part(b0, b1);
      row.patterns[p] = { start, end };
      if (end.peak > start.peak * 1.5 + 0.5) failures.push(`${sc.name} / ${p}: transitions at 5 min (${end.peak.toFixed(2)}) much rougher than at the start (${start.peak.toFixed(2)})`);
      for (const k of ["peak", "path"]) if (end[k] > bound(p, k)) failures.push(`${sc.name} / ${p} at 5 min: ${k} ${end[k].toFixed(2)} > ${bound(p, k)}`);
    } else {
      const s = score(run(p, changes, { total: seconds }));
      row.patterns[p] = s;
      for (const k of ["peak", "path"]) if (s[k] > bound(p, k)) failures.push(`${sc.name} / ${p}: ${k} ${s[k].toFixed(2)} > ${bound(p, k)}`);
    }
  }
  report.scenarios.push(row);
  console.log(`${sc.name.padEnd(44)} answers ${answers}, speaking stretches ${entries}`);
}

/** The changes from `from` seconds on, rebased to start at 0 with the state current then. */
function rebase(changes, from) {
  const before = changes.filter((c) => c[0] <= from).at(-1) ?? changes[0];
  return [[0, before[1]], ...changes.filter((c) => c[0] > from).map(([t, s]) => [t - from, s])];
}

if (jsonOut) writeFileSync(jsonOut, JSON.stringify(report, null, 1));
if (failures.length) {
  console.error(`\ntransitions: ${failures.length} bound(s) crossed:\n  ${failures.join("\n  ")}`);
  process.exit(1);
}
console.log("\ntransitions: every pattern and conversation within the contract's bounds");
