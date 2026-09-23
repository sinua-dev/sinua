// Materials cross-platform check, Android collection: rebuild the PNGs the
// instrumented render test emitted.
//
//   node collect-android.mjs <logcat file | androidTest-results dir> <out dir>
//
// Why logcat at all: `renderMaterialsGoldenCases` writes into the *test* APK's
// files, which are deleted with it, so the renders leave the device as base64
// chunks on the `FxMaterials` tag -- `name|i|chunk` repeated, then
// `name|END|<base64 length>`.
//
// Why Gradle's own per-test file rather than `adb logcat`: the README's recipe
// scrapes the live buffer, which on a shared emulator means reading from a
// start timestamp and hoping the ring buffer didn't wrap (1.3 MB of chunks
// against a 256 KB default buffer). Gradle already writes the whole thing to
// `<module>/build/outputs/androidTest-results/connected/debug/<AVD>/
// logcat-<class>-<method>.txt`, which is complete, deterministic, needs no adb
// and is an ordinary CI artifact.
//
// The `|END|<len>` marker is checked, not just parsed. It exists because
// truncation is the expected failure here, and until now nothing read it: a
// short PNG would simply have been a corrupt or absent file, which compare.cjs
// then reported as "missing" and passed.
import { readFileSync, writeFileSync, mkdirSync, readdirSync, statSync } from "node:fs";
import { join } from "node:path";

const TAG = "FxMaterials: ";

/** Accepts the logcat file itself, or any directory containing exactly one. */
function resolveLog(input) {
  if (statSync(input).isFile()) return input;
  const hits = [];
  const walk = (dir) => {
    for (const e of readdirSync(dir, { withFileTypes: true })) {
      const p = join(dir, e.name);
      if (e.isDirectory()) walk(p);
      else if (/^logcat-.*renderMaterialsGoldenCases\.txt$/.test(e.name)) hits.push(p);
    }
  };
  walk(input);
  if (hits.length !== 1) {
    fail(
      hits.length
        ? `${hits.length} materials logcat files under ${input}; pass the one you mean:\n  ${hits.join("\n  ")}`
        : `no logcat-*renderMaterialsGoldenCases.txt under ${input} -- did the instrumented test run?`
    );
  }
  return hits[0];
}

function fail(message) {
  console.error(`collect-android: ${message}`);
  process.exit(1);
}

const [input, outDir] = process.argv.slice(2);
if (!input || !outDir) fail("usage: collect-android.mjs <logcat file | results dir> <out dir>");

const logPath = resolveLog(input);
const chunks = new Map(); // name -> array of chunks by index
const ends = new Map(); // name -> declared base64 length

for (const line of readFileSync(logPath, "utf8").split("\n")) {
  const at = line.indexOf(TAG);
  if (at < 0) continue;
  const payload = line.slice(at + TAG.length);
  const first = payload.indexOf("|");
  const second = payload.indexOf("|", first + 1);
  if (first < 0 || second < 0) continue;
  const name = payload.slice(0, first);
  const index = payload.slice(first + 1, second);
  const rest = payload.slice(second + 1);
  if (index === "END") {
    ends.set(name, Number(rest));
    continue;
  }
  if (!chunks.has(name)) chunks.set(name, []);
  chunks.get(name)[Number(index)] = rest;
}

if (ends.size === 0) {
  fail(`no FxMaterials output in ${logPath} -- the test ran but emitted nothing, or the tag changed`);
}

mkdirSync(outDir, { recursive: true });
const problems = [];
let written = 0;
for (const [name, declared] of ends) {
  const parts = chunks.get(name) ?? [];
  const missing = [];
  for (let i = 0; i < parts.length; i++) if (parts[i] === undefined) missing.push(i);
  const b64 = parts.join("");
  if (missing.length) {
    problems.push(`${name}: chunk(s) ${missing.join(", ")} never arrived (logcat truncated?)`);
    continue;
  }
  if (b64.length !== declared) {
    problems.push(`${name}: reassembled ${b64.length} base64 chars, the test declared ${declared}`);
    continue;
  }
  writeFileSync(join(outDir, name), Buffer.from(b64, "base64"));
  written++;
}
// A name that chunked but never reached END is a truncation too, and would
// otherwise be silently dropped -- nothing above iterates it.
for (const name of chunks.keys()) {
  if (!ends.has(name)) problems.push(`${name}: chunks but no END marker (logcat truncated?)`);
}

console.log(`collect-android: ${written} render(s) -> ${outDir}  (from ${logPath})`);
if (problems.length) {
  console.error(`collect-android: ${problems.length} incomplete render(s):`);
  for (const p of problems) console.error(`  ${p}`);
  process.exit(1);
}
