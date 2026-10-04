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

// 1 = a perfect transition. Each bound sits ~10-15 % over what the contract measures
// (design note 31, "Sınırlar: ölçülen / sınır"), so a regression shows; the exceptions are
// motion that is meant to travel, each with its measured value.
const GENERAL = { peak: 2.7, path: 2.25 }; // measured 2.39 (glowing, a 5-minute session) / 1.99 (chirp)
const RING = { path: 3.3, why: "thinking starts the ring's own brightness pulse with the change (measured 2.93)" };
const EXCEPT = {
  speaking: { peak: 3.0, path: 5.8, why: "the spectrum's bars join one at a time and slide, the user's pick (measured 2.64 / 5.12)" },
  concluding: { path: 4.7, why: "crystallize assembles and scatters on its own cycle (measured 4.18)" },
  muted: { peak: 6.8, path: 5.4, why: "the sphere turns (yaw 1.2 / 1.0 / none per state): a rigid turn of a dense lattice (measured 6.07 / 4.79)" },
  calibrating: { peak: 12.2, why: "chladni snaps to a new mode every holdDuration; smoothing it is a 1.14 candidate (measured 10.85)" },
  beep: { peak: 3.7, path: 3.3, why: "the arms swing out wide, a big continuous move (measured 3.33 / 2.91)" },
  completing: RING,
  tracking: RING,
  stepping: RING,
  measuring: RING,
  playing: { path: 2.5, why: "playback steps (measured 2.19)" },
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

// Conversations: each answer is exactly one speaking stretch (the adaptive tail holds every
// pause the scripts leave, design note 31 V7), and the transitions stay inside the bounds on
// representative patterns, as smooth at the end of a 5-minute session as at its start.
const SAMPLE = quick ? ["glowing"] : ["glowing", "working", "breathing", "tracking", "speaking", "buzzy"];
for (const sc of SCENARIOS) {
  const { changes, answers, seconds } = states(sc);
  const entries = changes.filter((c) => c[1] === "speaking").length;
  if (entries !== answers) failures.push(`${sc.name}: ${entries} speaking stretches for ${answers} answers`);
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
