// The external oracle for the orbs family: upstream `thinking-orbs`' own engine,
// run unmodified, at any size -- including 32, which upstream's frozen golden
// (spec/orbs-golden.json, 64 and 20 only) never covered.
//
// This is upstream's scripts/extract-golden.ts with two differences, both
// outside upstream's code: the sizes are an argument, and the output carries
// where it came from. It imports upstream's `resolvePreset`, `STATE_TO_MODE`
// and `MODE_FRAMES` from a checkout you point it at; nothing upstream is edited.
//
// It proves itself: run with `--sizes 64,20` it must reproduce the vendored
// spec/orbs-golden.json byte for byte. That is what makes a later upstream
// commit a valid oracle for sizes the vendored file never had.
//
// Run (docs: scripts/oracle/README.md):
//   npx tsx scripts/oracle/extract-orbs-golden.ts <thinking-orbs dir> --sizes 32 > spec/orbs-golden-32.json
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { pathToFileURL } from "node:url";
import { execFileSync } from "node:child_process";

const [dirArg, flag, sizesArg] = process.argv.slice(2);
if (!dirArg || flag !== "--sizes" || !sizesArg) {
  console.error("usage: extract-orbs-golden.ts <thinking-orbs package dir> --sizes 64,20|32");
  process.exit(2);
}
const dir = resolve(dirArg);
const SIZES = sizesArg.split(",").map(Number);
const from = (p: string) => import(pathToFileURL(join(dir, p)).href);
const { MODE_FRAMES } = await from("src/engine/registry.ts");
const { resolvePreset, STATE_TO_MODE } = await from("src/presets.ts");
const pkg = JSON.parse(readFileSync(join(dir, "package.json"), "utf8"));

// Upstream's loop, times and rounding, verbatim (scripts/extract-golden.ts).
const STATES = Object.keys(STATE_TO_MODE);
const TIMES = [0.6, 1.7, 3.3, 5.1];
const P = 6;
const r6 = (n: number) => Number(n.toFixed(P));

const cases = [];
for (const state of STATES) {
  for (const size of SIZES) {
    const { mode, opts } = resolvePreset(state, size);
    for (const t of TIMES) {
      const frame = MODE_FRAMES[mode](size, t, opts);
      cases.push({
        key: `${state}-${size}-${t}`,
        state,
        size,
        mode,
        t,
        dotCount: frame.dots.length,
        lineCount: frame.lines.length,
        dots: frame.dots.flatMap((d) => [r6(d.x), r6(d.y), r6(d.z), r6(d.r), r6(d.white), r6(d.a ?? 1)]),
        lines: frame.lines.flatMap((l) => [r6(l.x1), r6(l.y1), r6(l.x2), r6(l.y2), r6(l.white), r6(l.a ?? 1), r6(l.w)]),
      });
    }
  }
}

const golden: Record<string, unknown> = {
  specVersion: "1.0.0",
  sourceLibrary: { name: "thinking-orbs", version: pkg.version },
  note: "Dot stride 6: x, y, z, r, white, a. Line stride 7: x1, y1, x2, y2, white, a, w. Dots are in draw order (z ascending); lines draw first.",
  tolerance: 1e-4,
  times: TIMES,
  resolved: Object.fromEntries(
    STATES.flatMap((state) =>
      SIZES.map((size) => {
        const { mode, speed, opts } = resolvePreset(state, size);
        return [`${state}-${size}`, { mode, speed, opts }];
      }),
    ),
  ),
  cases,
};

// Provenance only for sizes the vendored file doesn't have: at 64,20 the output
// must equal the vendored file exactly, so it gets no extra key.
if (SIZES.join() !== "64,20") {
  const git = (...a: string[]) => execFileSync("git", ["-C", dir, ...a], { encoding: "utf8" }).trim();
  golden.provenance = {
    repo: "https://github.com/Jakubantalik/Libraries.dev",
    path: "packages/thinking-orbs",
    commit: git("rev-parse", "HEAD"),
    clean: git("status", "--porcelain", "--", ".") === "",
    harness: "scripts/oracle/extract-orbs-golden.ts (upstream's extract-golden.ts loop, sizes as an argument)",
    validity:
      "the same commit and harness run with --sizes 64,20 reproduce spec/orbs-golden.json byte for byte (sha256 70bfaa2bbf1390b63f6ec02f16d7a4329fee67b3b290e671fe1d205328165e20)",
  };
}

process.stdout.write(`${JSON.stringify(golden)}\n`);
