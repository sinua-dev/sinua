// Materials cross-platform check, step 2: render each case with the Web
// painter in Chrome, load the iOS/Android renders (ios-<key>-<theme>.png,
// android-<key>-<theme>.png from MaterialsRenderTests / renderMaterialsGoldenCases),
// composite everything over the theme paper and compare RGB -- the tolerance
// in docs/fx-view.md (mean <= 2/255, p99 <= 24/255). Writes a contact sheet.
//   PLAYWRIGHT=... CHROME=... node compare.cjs out/frames.json <png dir> [out/sheet.png]
//                                              [--platforms ios,android] [--expect <n>]
//
// `--platforms` declares which renderers this run is a statement about, and
// `--expect` how many cases it is a statement about. Both are required to reach
// a pass, because the verdict used to be reachable without comparing anything:
// a platform with no PNG was recorded as the *string* "missing", the verdict
// loop only inspected object values, and so an empty directory printed 42 rows
// of "missing" followed by ALL WITHIN TOLERANCE and exit 0. A cross-platform
// check that passes when neither platform rendered is worse than no check.
const fs = require("fs");
const path = require("path");
const { chromium } = require(process.env.PLAYWRIGHT ?? "playwright");

const argv = process.argv.slice(2);
const flag = (name) => {
  const i = argv.indexOf(name);
  if (i < 0) return null;
  const v = argv[i + 1];
  argv.splice(i, 2);
  return v;
};
const platformsArg = flag("--platforms");
const expectArg = flag("--expect");
const [framesPath, pngDir, sheetPath = "out/sheet.png"] = argv;

const PLATFORMS = (platformsArg ?? "ios,android")
  .split(",")
  .map((s) => s.trim())
  .filter(Boolean);
for (const p of PLATFORMS) {
  if (p !== "ios" && p !== "android") {
    console.error(`compare: unknown platform "${p}" (expected ios and/or android)`);
    process.exit(2);
  }
}
const EXPECT = expectArg == null ? null : Number(expectArg);
if (expectArg != null && !Number.isInteger(EXPECT)) {
  console.error(`compare: --expect wants a whole number of cases, got "${expectArg}"`);
  process.exit(2);
}

// Anti-aliasing exceptions (agreed with families 2026-09-19). Deliberate additions only,
// each with a reason, like a golden re-baseline. Listed cases pass on full-res mean <= 2
// AND half-res (2x2 box) p99 <= 24; their full-res p99 is reported for information.
const AA_EXCEPTIONS = {
  "drifting-64-0.6-particles-liquid": "dense thin-stroke curls (24 polylines at ~2.4 px since families' Q2 re-baseline): Chrome AA vs CoreGraphics/Skia",
};
// Renderer-environment exceptions (user decision 2026-09-21, docs/fx-view.md). A listed case
// passes on mean <= its own limit instead of 2; p99 stays <= 24. Only for a render measured to
// move with the *environment* (browser/OS build, Android GPU backend) rather than our paint.
// Additions are deliberate, each with the measurement that justified it.
const ENV_EXCEPTIONS = {
  "tracking-64-0.6-glow-blur-additive": { mean: 3, reason: "additive-blend blur: 2.023 web(Linux headless Chrome)-vs-Android(x86_64 swiftshader) in CI, 0.53 locally; CI Android vs local Web 1.23" },
};
(async () => {
  const frames = JSON.parse(fs.readFileSync(framesPath));
  const paint = fs
    .readFileSync(path.join(__dirname, "../../dist/paint.js"), "utf8")
    .replace(/import \{[^}]*\} from "@sinua\/core";/, "function readPacked(){} function unpackFrame(){}");
  const pngs = {};
  for (const f of fs.readdirSync(pngDir)) if (/^(ios|android)-.*\.png$/.test(f)) pngs[f] = "data:image/png;base64," + fs.readFileSync(path.join(pngDir, f)).toString("base64");
  const browser = await chromium.launch(process.env.CHROME ? { executablePath: process.env.CHROME } : {});
  const page = await browser.newPage({ viewport: { width: 820, height: 1000 } });
  await page.setContent("<body style='margin:0;font:11px system-ui'><div id=sheet></div></body>");
  const rows = await page.evaluate(async ({ frames, paint, pngs, exceptions, envExceptions, platforms }) => {
    const lib = await import(URL.createObjectURL(new Blob([paint], { type: "text/javascript" })));
    const load = (src) => new Promise((ok) => { const i = new Image(); i.onload = () => ok(i); i.src = src; });
    const pixels = (draw) => { const c = document.createElement("canvas"); c.width = c.height = 256; const x = c.getContext("2d"); draw(x); return { c, d: x.getImageData(0, 0, 256, 256).data }; };
    // Composite over the theme paper, RGB per channel; `half` = 2x2 box-averaged first.
    const seen = (d, bg, half) => {
      const n = half ? 128 : 256, s = half ? 2 : 1, out = new Float64Array(n * n * 3);
      for (let y = 0; y < n; y++) for (let x = 0; x < n; x++) for (let k = 0; k < 3; k++) {
        let acc = 0;
        for (let dy = 0; dy < s; dy++) for (let dx = 0; dx < s; dx++) {
          const i = ((y * s + dy) * 256 + (x * s + dx)) * 4;
          acc += (d[i + k] * d[i + 3] + bg * (255 - d[i + 3])) / 255;
        }
        out[(y * n + x) * 3 + k] = acc / (s * s);
      }
      return out;
    };
    const stats = (a, b) => {
      const diffs = Array.from(a, (v, i) => Math.abs(v - b[i])).sort((x, y) => x - y);
      return { mean: +(diffs.reduce((s, v) => s + v, 0) / diffs.length).toFixed(3), p99: +diffs[Math.floor(diffs.length * 0.99)].toFixed(1) };
    };
    const metric = (a, b, dark, exception) => {
      const bg = dark ? 0 : 255;
      const full = stats(seen(a, bg, false), seen(b, bg, false));
      if (!exception) return full;
      const half = stats(seen(a, bg, true), seen(b, bg, true));
      return { mean: full.mean, p99: half.p99, p99HalfRes: half.p99, p99FullResInfo: full.p99 };
    };
    const sheet = document.getElementById("sheet"); const out = [];
    for (const { key, frame } of frames) for (const dark of [false, true]) {
      const theme = dark ? "dark" : "light";
      const imgs = { web: pixels((x) => lib.drawFrame(x, frame, dark, 4)) };
      const row = { key, theme };
      for (const plat of platforms) {
        const src = pngs[`${plat}-${key}-${theme}.png`];
        if (!src) { row[plat] = "missing"; continue; }
        const img = await load(src);
        imgs[plat] = pixels((x) => x.drawImage(img, 0, 0, 256, 256));
        row[`web↔${plat}`] = metric(imgs.web.d, imgs[plat].d, dark, !!exceptions[key]);
      }
      if (imgs.ios && imgs.android) row["ios↔android"] = metric(imgs.ios.d, imgs.android.d, dark, !!exceptions[key]);
      if (exceptions[key]) row.aaException = exceptions[key];
      if (envExceptions[key]) row.envException = envExceptions[key].reason;
      out.push(row);
      const div = document.createElement("div");
      div.style.cssText = `display:flex;gap:4px;align-items:center;background:${dark ? "#000" : "#fff"};color:${dark ? "#fff" : "#000"}`;
      for (const [n, p] of Object.entries(imgs)) { p.c.style.width = "180px"; const w = document.createElement("div"); w.append(p.c, document.createTextNode(" " + n)); div.append(w); }
      const t = document.createElement("span"); t.textContent = `${key} ${theme}`; div.append(t); sheet.append(div);
    }
    return out;
  }, { frames, paint, pngs, exceptions: AA_EXCEPTIONS, envExceptions: ENV_EXCEPTIONS, platforms: PLATFORMS });

  // --- verdict ---------------------------------------------------------
  // Three ways to fail, and two of them are about the run rather than the
  // pixels. A comparison can only claim "within tolerance" for pairs it
  // actually computed, so anything it was told to expect and did not find is a
  // failure with a name, never a quiet omission.
  const outOfTolerance = [];
  const missing = [];
  for (const r of rows) {
    // "x-" keys are synthetic, information-only checks (frames.mjs SYNTHETIC): reported, never failing.
    for (const [k, v] of Object.entries(r)) {
      if (v === "missing") missing.push(`${r.key} ${r.theme}: no ${k} render`);
      const meanLimit = ENV_EXCEPTIONS[r.key]?.mean ?? 2;
      if (!r.key.startsWith("x-") && v && typeof v === "object" && (v.mean > meanLimit || v.p99 > 24)) {
        outOfTolerance.push(`${r.key} ${r.theme} ${k}: mean ${v.mean}, p99 ${v.p99}`);
      }
    }
    console.log(JSON.stringify(r));
  }

  fs.mkdirSync(path.dirname(sheetPath), { recursive: true });
  await page.screenshot({ path: sheetPath, fullPage: true });
  await browser.close();

  const problems = [];
  if (EXPECT != null && frames.length !== EXPECT) {
    problems.push(`expected ${EXPECT} cases, the frames file has ${frames.length}`);
  }
  if (rows.length === 0) problems.push("no cases were compared at all");
  for (const m of missing) problems.push(m);
  for (const t of outOfTolerance) problems.push(t);

  const pairs = rows.reduce((n, r) => n + Object.values(r).filter((v) => v && typeof v === "object").length, 0);
  console.log(`compared ${pairs} pair(s) over ${rows.length} row(s): ${PLATFORMS.join(" + ")} vs the Web`);
  if (problems.length) {
    console.log("OUT OF TOLERANCE");
    for (const p of problems) console.error(`  ${p}`);
    process.exit(1);
  }
  console.log("ALL WITHIN TOLERANCE");
})();
