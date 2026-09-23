#!/usr/bin/env node
// sinua bench report (docs/bench.md). No dependencies.
//
//   node scripts/bench/report.mjs result.json [more.json ...]   tables + cost check
//   node scripts/bench/report.mjs --check result.json ...        schema check only
//   node scripts/bench/report.mjs --compare a.json b.json        per-case delta b vs a
//
// Checks each file against spec/bench/result.schema.json's required fields
// and types (a small subset validator, enough for this schema).
import { readFileSync } from "node:fs";
import { fileURLToPath } from "node:url";

const schema = JSON.parse(readFileSync(fileURLToPath(new URL("../../spec/bench/result.schema.json", import.meta.url)), "utf8"));
const args = process.argv.slice(2);
const mode = args[0] === "--check" || args[0] === "--compare" ? args.shift() : "report";
if (args.length === 0) {
  console.error("usage: report.mjs [--check|--compare] result.json ...");
  process.exit(2);
}

function check(node, s, path, errs) {
  if (s.$ref) s = schema.$defs[s.$ref.split("/").pop()];
  if (s.const !== undefined && node !== s.const) errs.push(`${path}: expected ${s.const}`);
  if (s.enum && !s.enum.includes(node)) errs.push(`${path}: ${JSON.stringify(node)} not in ${s.enum.join("|")}`);
  const types = s.type ? [].concat(s.type) : null;
  if (types) {
    const t = node === null ? "null" : Array.isArray(node) ? "array" : Number.isInteger(node) ? "integer" : typeof node;
    const ok = types.some((x) => x === t || (x === "number" && t === "integer"));
    if (!ok) return errs.push(`${path}: type ${t}, expected ${types.join("|")}`);
  }
  if (s.required) for (const k of s.required) if (!(node && k in node)) errs.push(`${path}: missing ${k}`);
  if (s.properties && node && typeof node === "object") {
    for (const [k, sub] of Object.entries(s.properties)) if (k in node) check(node[k], sub, `${path}.${k}`, errs);
  }
  if (s.items && Array.isArray(node)) node.forEach((x, i) => check(x, s.items, `${path}[${i}]`, errs));
  return errs;
}

const files = args.map((f) => {
  const r = JSON.parse(readFileSync(f, "utf8"));
  const errs = check(r, schema, "$", []);
  if (errs.length) {
    console.error(`${f}: ${errs.length} schema error(s)\n  ` + errs.slice(0, 20).join("\n  "));
    process.exitCode = 1;
  } else console.log(`${f}: schema ok (${r.platform}, ${r.cases.length} runs)`);
  return { f, r };
});
if (mode === "--check" || process.exitCode) process.exit(process.exitCode ?? 0);

const hitchFlag = (x) => (x <= 5 ? "good" : x <= 10 ? "warn" : "CRIT"); // Apple's hitch time ratio bands, ms/s
const pad = (s, n) => String(s).padEnd(n);
const num = (x, d = 1) => (x == null ? "-" : Number(x).toFixed(d)).padStart(7);

// Spearman rank correlation (average ranks for ties).
function ranks(xs) {
  const idx = xs.map((x, i) => [x, i]).sort((a, b) => a[0] - b[0]);
  const r = new Array(xs.length);
  for (let i = 0; i < idx.length; ) {
    let j = i;
    while (j + 1 < idx.length && idx[j + 1][0] === idx[i][0]) j++;
    for (let k = i; k <= j; k++) r[idx[k][1]] = (i + j) / 2 + 1;
    i = j + 1;
  }
  return r;
}
function spearman(a, b) {
  const ra = ranks(a), rb = ranks(b), n = a.length;
  const ma = ra.reduce((s, x) => s + x, 0) / n, mb = rb.reduce((s, x) => s + x, 0) / n;
  let num = 0, da = 0, db = 0;
  for (let i = 0; i < n; i++) { num += (ra[i] - ma) * (rb[i] - mb); da += (ra[i] - ma) ** 2; db += (rb[i] - mb) ** 2; }
  return da && db ? num / Math.sqrt(da * db) : 0;
}

if (mode === "--compare") {
  const [a, b] = files;
  console.log(`\nB vs A: ${b.r.platform} ${b.r.device.model} vs ${a.r.platform} ${a.r.device.model}`);
  console.log(pad("case", 18) + pad("power", 7) + "   fps A   fps B  p95 A   p95 B  work A  work B");
  for (const cb of b.r.cases) {
    const ca = a.r.cases.find((x) => x.id === cb.id && x.power === cb.power);
    if (!ca) continue;
    const w = (c) => c.computeMs.p95 + c.paintMs.p95;
    console.log(pad(cb.id, 18) + pad(cb.power, 7) + num(ca.fps) + num(cb.fps) + num(ca.frameMs.p95) + num(cb.frameMs.p95) + num(w(ca), 2) + num(w(cb), 2));
  }
  process.exit(0);
}

for (const { r } of files) {
  const label = r.device.isSimulator ? "SIMULATOR / EMULATOR / DESKTOP: not device numbers" : "device";
  console.log(`\n== ${r.platform} · ${r.device.model} · ${r.device.os.slice(0, 60)} · ${r.device.refreshHz} Hz · ${r.measureSeconds} s/case · ${label}`);
  console.log(pad("case", 18) + pad("power", 7) + "    fps  f p50  f p95  f p99  comp95 paint95 dropped  hitch ms/s    cpu%  cost");
  for (const c of r.cases) {
    console.log(
      pad(c.id, 18) + pad(c.power, 7) + num(c.fps) + num(c.frameMs.p50) + num(c.frameMs.p95) + num(c.frameMs.p99) +
      num(c.computeMs.p95, 2) + num(c.paintMs.p95, 2) + String(c.droppedFrames).padStart(8) +
      `${num(c.hitchRatioMsPerS)} ${hitchFlag(c.hitchRatioMsPerS)}` + num(c.cpuPct) + `  ${c.cost?.class ?? "-"} (${c.cost?.elements ?? "-"} el, blur ${c.cost?.blurLoad ?? "-"})`,
    );
  }
  // Does the cost badge predict the measured work? Normal power, work = compute p95 + paint p95.
  const normal = r.cases.filter((c) => c.power === "normal" && c.cost);
  if (normal.length >= 3) {
    const work = normal.map((c) => c.computeMs.p95 + c.paintMs.p95);
    const rhoEl = spearman(normal.map((c) => c.cost.elements), work);
    const rhoBlur = spearman(normal.map((c) => c.cost.elements + 100 * c.cost.blurLoad), work);
    console.log(`cost vs measured work (normal power, Spearman rho): elements ${rhoEl.toFixed(2)}, elements+blur ${rhoBlur.toFixed(2)} (1 = the badge ranks cases exactly as measured)`);
    const byWork = [...normal].sort((a, b) => b.computeMs.p95 + b.paintMs.p95 - (a.computeMs.p95 + a.paintMs.p95));
    const cls = { light: 0, medium: 1, heavy: 2 };
    const inversions = [];
    for (const hi of byWork) for (const lo of normal) {
      if (cls[lo.cost.class] > cls[hi.cost.class] && hi.computeMs.p95 + hi.paintMs.p95 > 1.5 * (lo.computeMs.p95 + lo.paintMs.p95)) inversions.push(`${hi.id} (${hi.cost.class}) measures > 1.5x ${lo.id} (${lo.cost.class})`);
    }
    console.log(inversions.length ? "badge inversions:\n  " + inversions.join("\n  ") : "badge inversions: none");
  }
}
