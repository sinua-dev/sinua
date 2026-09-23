// Extracts a handful of checkpoints from spec/sinua-golden.json for the
// on-device RN smoke test (the app can't read the repo file, and bundling
// the full ~0.9 MB set into the app isn't worth it). Re-run after every
// deliberate re-baseline of spec/sinua-golden.json:
//   node packages/react-native/example/scripts/gen-sinua-checkpoints.mjs
// packages/core/test/sinua-checkpoints.test.mjs fails if the two drift.
import { readFileSync, writeFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

/** Which cases, and why (orchestrator 2026-09-18: one of each kind). */
export const CHECKPOINT_KEYS = [
  "scrolling-64-0.6", // polyline-heavy (40 stroked pills)
  "metering-64-0.6", // dot-heavy (96 LEDs)
  "glowing-64-0.6", // coloured additive orb (saturation/hue through the bridge)
  "tracking-64-0.6-laps", // input-driven (overrides; lap + halo polylines)
  "working-64-0.6-color-fixed", // Color system: apply_color + colorMode "fixed" through the bridge
];

export function buildCheckpoints(golden) {
  const byKey = new Map(golden.cases.map((c) => [c.key, c]));
  return {
    note: "Generated from spec/sinua-golden.json by scripts/gen-sinua-checkpoints.mjs -- do not edit by hand.",
    tolerance: golden.tolerance,
    cases: CHECKPOINT_KEYS.map((key) => {
      const c = byKey.get(key);
      if (!c) throw new Error(`${key} missing from spec/sinua-golden.json`);
      const poly = c.polylines;
      return {
        key,
        state: c.state,
        size: c.size,
        t: c.t,
        overrides: c.overrides,
        dotCount: c.dotCount,
        lineCount: c.lineCount,
        polylineCount: c.polylineCount,
        colorMode: c.colorMode,
        firstDot: c.dotCount ? c.dots.slice(0, 8) : null,
        lastDot: c.dotCount ? c.dots.slice(c.dots.length - 8) : null,
        firstPolyline: poly.length ? { style: poly[0].style, first: poly[0].points.slice(0, 2), last: poly[0].points.slice(-2) } : null,
        lastPolyline: poly.length
          ? { style: poly[poly.length - 1].style, first: poly[poly.length - 1].points.slice(0, 2), last: poly[poly.length - 1].points.slice(-2) }
          : null,
      };
    }),
  };
}

const here = (p) => fileURLToPath(new URL(p, import.meta.url));
if (process.argv[1] === fileURLToPath(import.meta.url)) {
  const golden = JSON.parse(readFileSync(here("../../../../spec/sinua-golden.json"), "utf8"));
  writeFileSync(here("../sinuaCheckpoints.json"), JSON.stringify(buildCheckpoints(golden), null, 2) + "\n");
  console.log("wrote example/sinuaCheckpoints.json");
}
