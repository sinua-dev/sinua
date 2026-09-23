// Materials cross-platform check, iOS collection: pull the render attachments
// out of an .xcresult and give them back the names the test chose.
//
//   node collect-ios.mjs <path to .xcresult> <out dir>
//
// `MaterialsRenderTests` attaches each render as `ios-<key>-<theme>.png`, but
// `xcresulttool export attachments` writes them under UUID filenames and puts
// the intended name in `manifest.json` as `suggestedHumanReadableName` --
// suffixed with an occurrence index and another UUID:
//
//   ios-glowing-64-0.6-holo-dark_0_5C3961D0-…-110E6EB1A86A.png
//
// Stripping that suffix is the whole job, and it is what the README asked a
// human to do by hand.
import { execFileSync } from "node:child_process";
import { readFileSync, mkdirSync, mkdtempSync, renameSync, rmSync, existsSync, readdirSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";

function fail(message) {
  console.error(`collect-ios: ${message}`);
  process.exit(1);
}

const [resultBundle, outDir] = process.argv.slice(2);
if (!resultBundle || !outDir) fail("usage: collect-ios.mjs <R.xcresult> <out dir>");
if (!existsSync(resultBundle)) fail(`no result bundle at ${resultBundle} -- did xcodebuild run with -resultBundlePath?`);

mkdirSync(outDir, { recursive: true });
// Export into a scratch directory rather than straight into `outDir`, for two
// reasons found by running this twice: `xcresulttool` refuses with "Failed to
// generate manifest.json: file already exists" if one is there from a previous
// run, and `outDir` is shared with the Android renders, which have no business
// being next to a manifest and 42 UUID-named files.
const stage = mkdtempSync(join(tmpdir(), "collect-ios-"));
try {
  execFileSync("xcrun", ["xcresulttool", "export", "attachments", "--path", resultBundle, "--output-path", stage], {
    stdio: ["ignore", "ignore", "pipe"],
  });
} catch (err) {
  fail(`xcresulttool export failed: ${String(err.stderr ?? err).trim()}`);
}

const manifestPath = join(stage, "manifest.json");
if (!existsSync(manifestPath)) fail(`xcresulttool wrote no manifest.json into ${stage}`);
const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));

// `_<occurrence>_<UUID>.png`, appended by xcresulttool, not by the test.
const SUFFIX = /_\d+_[0-9A-Fa-f-]{36}\.png$/;
const problems = [];
let renamed = 0;
for (const entry of manifest) {
  for (const a of entry.attachments ?? []) {
    const suggested = a.suggestedHumanReadableName ?? "";
    if (!suggested.startsWith("ios-")) continue; // other attachments (screenshots, logs)
    const name = suggested.replace(SUFFIX, ".png");
    if (!name.endsWith(".png")) {
      problems.push(`${suggested}: not a png name after stripping the export suffix`);
      continue;
    }
    const from = join(stage, a.exportedFileName);
    if (!existsSync(from)) {
      problems.push(`${name}: manifest lists ${a.exportedFileName}, which xcresulttool did not write`);
      continue;
    }
    renameSync(from, join(outDir, name));
    renamed++;
  }
}

if (renamed === 0) {
  fail(
    `no ios-*.png attachments in ${resultBundle} -- MaterialsRenderTests produced no renders ` +
      `(check -only-testing and that the test actually ran)`
  );
}
console.log(`collect-ios: ${renamed} render(s) -> ${outDir}`);
if (problems.length) {
  console.error(`collect-ios: ${problems.length} problem(s):`);
  for (const p of problems) console.error(`  ${p}`);
  process.exit(1);
}
// Leftover UUID files mean the manifest and the export disagree; say so rather
// than leaving them to be silently ignored by the comparison's glob.
const leftovers = readdirSync(stage).filter((f) => /^[0-9A-Fa-f-]{36}\.png$/.test(f));
if (leftovers.length) console.warn(`collect-ios: ${leftovers.length} exported file(s) had no manifest entry`);
rmSync(stage, { recursive: true, force: true });
