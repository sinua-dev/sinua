// Generates spec/paint-ink-vectors.json: the painter's colour conversion,
// pinned once so Swift and Kotlin can be checked against the Web's answer.
//
// Why this file exists. `ink()` has three implementations, not one:
//
//   Web    packages/web/src/paint.ts        emits the CSS string
//                                           `hsla(h, round(s*100)%, round(l*100)%, a)`
//                                           -- the **browser** does the conversion, and
//                                           s/l are quantised to whole percent first
//   Swift  packages/ios/Sources/Sinua/FxPaint.swift
//                                           a **hand-written** hslToRgb, because SwiftUI's
//                                           Color(hue:saturation:brightness:) is HSB not HSL
//   Kotlin packages/android/view/.../FxPaint.kt
//                                           Color.hsl(...) -- **Compose's** conversion
//
// Until now nothing compared them. The same-language snapshot tests
// (OldOrbPaint.swift, OldPaint.kt) prove each painter has not changed against
// its own past, which is a different claim.
//
// The reference is **real Chrome**, not a re-derivation: the Web painter hands
// the browser a string, so the browser's resolution *is* the Web behaviour.
// A spec-faithful implementation runs alongside as a cross-check and any
// disagreement is reported rather than silently preferred -- the same method
// spec/voice-golden.json used against Chrome's AnalyserNode.
//
//   node packages/web/scripts/gen-ink-vectors.mjs [--check]
//
// Needs Playwright + a Chromium, as gen-voice-golden.mjs does:
//   PLAYWRIGHT=/path/to/node_modules/playwright node packages/web/scripts/gen-ink-vectors.mjs
import { createRequire } from "node:module";
import { readFileSync, writeFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const require = createRequire(import.meta.url);
const root = join(dirname(fileURLToPath(import.meta.url)), "../../..");
const OUT = join(root, "spec/paint-ink-vectors.json");
const check = process.argv.includes("--check");

const { ink } = await import(join(root, "packages/web/dist/paint.js"));

/**
 * The cases, chosen for the edges that actually bite rather than a bland grid:
 * the greyscale branch and the value just past it, hue at and around the wrap,
 * both ends of white (where dark-mirroring flips it), and alpha extremes.
 */
function cases() {
  const out = [];
  const whites = [0, 0.004, 0.25, 0.5, 0.73, 1];
  const sats = [0, 0.004, 0.25, 0.5, 1];
  const hues = [-30, 0, 45, 151, 200, 359, 360, 390];
  const alphas = [0, 0.37, 1];
  for (const dark of [false, true]) {
    for (const white of whites) {
      for (const saturation of sats) {
        for (const hue of saturation === 0 ? [0] : hues) {
          for (const alpha of saturation === 0 ? [1, 0.37] : alphas) {
            out.push({ white, saturation, hue, alpha, dark });
          }
        }
      }
    }
  }
  return out;
}

/**
 * CSS Color 4's hsl-to-rgb, applied to what `ink()` actually emits -- i.e.
 * after its integer-percent rounding. This is the cross-check, not the answer.
 */
function specRgba({ white, saturation, hue, alpha, dark }) {
  const w = Math.min(1, Math.max(0, white));
  const lightness = dark ? 1 - w : w;
  if (saturation <= 0) {
    const g = Math.round(lightness * 255);
    return [g, g, g, alpha];
  }
  const s = Math.round(Math.min(1, Math.max(0, saturation)) * 100) / 100;
  const l = Math.round(lightness * 100) / 100;
  const h = ((hue % 360) + 360) % 360;
  const f = (n) => {
    const k = (n + h / 30) % 12;
    const a = s * Math.min(l, 1 - l);
    return l - a * Math.max(-1, Math.min(k - 3, 9 - k, 1));
  };
  return [Math.round(f(0) * 255), Math.round(f(8) * 255), Math.round(f(4) * 255), alpha];
}

/** What Chrome resolves `ink(...)` to, as RGBA. */
async function chromeRgba(inputs) {
  const { chromium } = require(process.env.PLAYWRIGHT ?? "playwright");
  const browser = await chromium.launch(process.env.CHROME ? { executablePath: process.env.CHROME } : {});
  try {
    const page = await browser.newPage();
    const strings = inputs.map((c) => ink(c.white, c.saturation, c.hue, c.alpha, c.dark));
    return await page.evaluate((list) => {
      const el = document.createElement("div");
      document.body.appendChild(el);
      return list.map((css) => {
        el.style.color = "";
        el.style.color = css;
        const m = /rgba?\(([^)]+)\)/.exec(getComputedStyle(el).color);
        const parts = m[1].split(/[,/]/).map((x) => Number(x.trim()));
        return [parts[0], parts[1], parts[2], parts.length > 3 ? parts[3] : 1];
      });
    }, strings);
  } finally {
    await browser.close();
  }
}

const inputs = cases();
const chrome = await chromeRgba(inputs);

// Cross-check: report disagreement with its magnitude, never pick silently.
let worst = 0;
const disagreements = [];
inputs.forEach((c, i) => {
  const s = specRgba(c);
  const d = Math.max(...[0, 1, 2].map((k) => Math.abs(s[k] - chrome[i][k])));
  if (d > worst) worst = d;
  if (d > 1) disagreements.push({ ...c, spec: s, chrome: chrome[i], delta: d });
});

const vectors = {
  note:
    "The painter's ink() conversion, resolved by real Chrome. Swift (hand-written hslToRgb) " +
    "and Kotlin (Compose Color.hsl) are checked against these. Regenerate: " +
    "node packages/web/scripts/gen-ink-vectors.mjs",
  generated: new Date().toISOString().slice(0, 10),
  chromeVsSpecMaxChannelDelta: worst,
  cases: inputs.map((c, i) => ({ ...c, rgba: chrome[i] })),
};

const text = JSON.stringify(vectors, null, 2) + "\n";

if (check) {
  if (!existsSync(OUT)) {
    console.error("spec/paint-ink-vectors.json is missing -- run without --check");
    process.exit(1);
  }
  const current = JSON.parse(readFileSync(OUT, "utf8"));
  const same = JSON.stringify(current.cases) === JSON.stringify(vectors.cases);
  console.log(same ? "ink vectors: up to date" : "ink vectors: STALE -- regenerate");
  process.exit(same ? 0 : 1);
}

writeFileSync(OUT, text);
console.log(`ink vectors: ${vectors.cases.length} cases -> spec/paint-ink-vectors.json`);
console.log(`chrome vs CSS Color 4 reference: max channel delta ${worst}`);
if (disagreements.length) {
  console.log(`${disagreements.length} case(s) disagree by more than 1/255:`);
  for (const d of disagreements.slice(0, 5)) console.log("  ", JSON.stringify(d));
}
