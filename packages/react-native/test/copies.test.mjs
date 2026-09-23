// This package does not depend on packages/ios and packages/android -- build.sh
// *copies* their sources in, and package.json's `files` ships those copies. They
// reproduce exactly today, but until this file nothing checked that, so editing a
// source without re-running build.sh silently published stale code. That has
// happened: a stale copy broke the RN Android build and went unnoticed until
// someone built the example by hand, which is why scripts/ci-rn.sh refreshes the
// copies before every run -- that hides staleness rather than catching it.
//
// The fix for any failure here is always the same: re-run
// `packages/react-native/build.sh` (COPY_ONLY=1 skips the Rust cross-compiles).
//
// The Android copies are plain `cp -R`, so they must be byte-identical. The iOS
// copies are transformed -- module imports dropped, `SinuaVoice.` qualifiers
// stripped, vendor types demoted from `public` -- because the pod flattens several
// SwiftPM modules into one. Those transforms are restated below, which makes this
// a **two-sided lock**: change build.sh's transform without changing this table and
// the test fails loudly, which is the intended outcome, not drift.
//
// Measured 2026-09-20, because a transform that currently matches nothing would
// let this file pass trivially and nobody would know: dropping `import
// CoreEngine`/`import SinuaVoice` and demoting `public` are all load-bearing
// (removing either from the table fails 5 and 4 entries respectively). Two
// clauses are **no-ops today** and kept only because build.sh does them --
// `SinuaVoice.` has no occurrences in Sources/Sinua, and Sources/SinuaVoice has
// no `import CoreEngine`. If one of those ever starts firing, it is already
// covered here.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readdirSync, readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const pkg = join(dirname(fileURLToPath(import.meta.url)), "..");
const p = (...parts) => join(pkg, ...parts);

/** `Sources/Sinua/*`: the pod is one module, so the module imports go. (build.sh:69) */
const stripSinuaImports = (s) =>
  s
    .split("\n")
    .filter((l) => l !== "import CoreEngine" && l !== "import SinuaVoice")
    .join("\n")
    .replaceAll("SinuaVoice.", "");

/** Sources/SinuaVoiceTypes is folded into the same module, so its imports go too. (build.sh:68) */
const dropTypesImport = (s) =>
  s.split("\n").filter((l) => l !== "import SinuaVoiceTypes" && l !== "@_exported import SinuaVoiceTypes").join("\n");

/** `Sources/SinuaVoice/*`: CoreEngine and SinuaVoiceTypes are folded in. (build.sh:70) */
const stripCoreEngineImport = (s) => dropTypesImport(s.split("\n").filter((l) => l !== "import CoreEngine").join("\n"));

/** Vendor types would otherwise land in the pod's generated ObjC header. (build.sh:81) */
const demotePublic = (s) =>
  s
    .split("\n")
    .map((l) =>
      l
        .replace(/^public final class /, "final class ")
        .replace(/^public class /, "class ")
        .replace(/^public enum /, "enum ")
        .replace(/^public struct /, "struct "),
    )
    .join("\n");

/** LiveKit's module is `LiveKit` under SwiftPM but `LiveKitClient` under CocoaPods. (build.sh:91) */
const liveKitModule = (s) =>
  s.split("\n").map((l) => (l === "import LiveKit" ? "import LiveKitClient" : l)).join("\n");

const vendor = (s) => demotePublic(stripSinuaImports(s));

/** Mirrors packages/react-native/build.sh. `source` is the truth; `copy` is what ships. */
const COPIES = [
  // --- Android: plain `cp -R`, so byte-identical -------------------------------
  {
    copy: p("android/src/main/kotlin/dev/sinua/view"),
    source: p("../android/view/src/main/kotlin/dev/sinua/view"),
    ext: ".kt",
    // Reads R.styleable from :sinua-view's own resources, which this module does
    // not have; RN hosts the Compose SinuaView directly. (build.sh:39-40)
    exclude: ["SinuaViewLayout.kt"],
  },
  { copy: p("android/src/main/kotlin/dev/sinua/voice"), source: p("../android/src/main/kotlin/dev/sinua/voice"), ext: ".kt" },
  { copy: p("android/src/main/kotlin/dev/sinua/gemini"), source: p("../android/gemini/src/main/kotlin/dev/sinua/gemini"), ext: ".kt" },
  { copy: p("android/src/main/kotlin/dev/sinua/elevenlabs"), source: p("../android/elevenlabs/src/main/kotlin/dev/sinua/elevenlabs"), ext: ".kt" },
  { copy: p("android/src/main/kotlin/dev/sinua/websocket"), source: p("../android/websocket/src/main/kotlin/dev/sinua/websocket"), ext: ".kt" },
  { copy: p("android/src/livekit/kotlin/dev/sinua/livekit"), source: p("../android/livekit/src/main/kotlin/dev/sinua/livekit"), ext: ".kt" },
  { copy: p("android/src/openai/kotlin/dev/sinua/openai"), source: p("../android/openai/src/main/kotlin/dev/sinua/openai"), ext: ".kt" },

  // --- iOS: flattened into one Swift module, so transformed --------------------
  { copy: p("ios/Sinua"), source: p("../ios/Sources/Sinua"), ext: ".swift", transform: (s) => dropTypesImport(stripSinuaImports(s)) },
  // Foundation only, no module imports: a plain copy. (build.sh:71)
  { copy: p("ios/SinuaVoiceTypes"), source: p("../ios/Sources/SinuaVoiceTypes"), ext: ".swift" },
  { copy: p("ios/SinuaVoice"), source: p("../ios/Sources/SinuaVoice"), ext: ".swift", transform: stripCoreEngineImport },
  { copy: p("ios/Vendors/GeminiLive"), source: p("../ios/Sources/SinuaGeminiLive"), ext: ".swift", transform: vendor },
  { copy: p("ios/Vendors/ElevenLabs"), source: p("../ios/Sources/SinuaElevenLabs"), ext: ".swift", transform: vendor },
  { copy: p("ios/Vendors/LiveKit"), source: p("../ios-livekit/Sources/SinuaLiveKit"), ext: ".swift", transform: (s) => demotePublic(liveKitModule(stripSinuaImports(s))) },
  { copy: p("ios/Vendors/OpenAI"), source: p("../ios-openai/Sources/SinuaOpenAI"), ext: ".swift", transform: vendor },
];

const names = (dir, ext, exclude = []) =>
  readdirSync(dir)
    .filter((f) => f.endsWith(ext) && !exclude.includes(f))
    .sort();

const REFRESH = "re-run packages/react-native/build.sh (COPY_ONLY=1 skips the Rust cross-compiles)";

for (const entry of COPIES) {
  const label = entry.copy.slice(pkg.length + 1);

  test(`copy is in sync: ${label}`, (t) => {
    if (!existsSync(entry.source)) {
      return t.skip(`${entry.source} is not present -- run this from the repo, not an installed package`);
    }
    assert.ok(existsSync(entry.copy), `${label} is missing entirely -- ${REFRESH}`);

    // State the denominator before comparing it. Without this the test passes
    // vacuously whenever `ext` stops matching -- an extension rename, an
    // over-broad `exclude` -- because both sets are then empty, `deepEqual([],
    // [])` holds, and the content loop below runs zero times. Measured
    // 2026-09-20: flipping one entry's `ext` from `.kt` to `.ktx` left the
    // suite at 26/26 pass, comparing nothing. A check that counts failures but
    // never counts inputs cannot tell "all good" from "nothing ran".
    const expected = names(entry.source, entry.ext, entry.exclude);
    assert.ok(
      expected.length > 0,
      `${label}: no ${entry.ext} files under ${entry.source} -- the extension or the exclude list is wrong, ` +
        `and an empty set would make this comparison vacuous`,
    );

    // Both directions: a source added and not copied, a source deleted with the
    // copy left behind, and a stray file in the copy are all drift, and none of
    // them shows up in a content comparison.
    assert.deepEqual(
      names(entry.copy, entry.ext),
      expected,
      `${label} has a different set of files from its source -- ${REFRESH}`,
    );

    for (const f of names(entry.copy, entry.ext)) {
      const source = readFileSync(join(entry.source, f), "utf8");
      const expected = entry.transform ? entry.transform(source) : source;
      assert.equal(
        readFileSync(join(entry.copy, f), "utf8"),
        expected,
        `${label}/${f} differs from its source -- ${REFRESH}`,
      );
    }
  });
}

test("the excluded files are excluded on purpose, and still exist upstream", (t) => {
  const view = COPIES[0];
  if (!existsSync(view.source)) return t.skip("packages/android is not present");
  // If SinuaViewLayout.kt ever disappears upstream, this exclusion is stale and
  // the comment explaining it is lying.
  assert.ok(
    existsSync(join(view.source, "SinuaViewLayout.kt")),
    "SinuaViewLayout.kt is gone upstream -- drop the exclusion in build.sh and here",
  );
  assert.ok(
    !existsSync(join(view.copy, "SinuaViewLayout.kt")),
    `SinuaViewLayout.kt was copied in; it needs R.styleable this module lacks -- ${REFRESH}`,
  );
});

// The pod is one target: Xcode refuses two compiled sources with the same file
// name ("Filename "Exports.swift" used twice"), wherever they sit in the tree.
// The copies above can each be perfect and still collide once flattened -- CI's
// pod build caught exactly that on 2026-09-21 (Sources/Sinua and Sources/SinuaVoice
// each had an Exports.swift). Counts every compiled source the podspec and its
// subspecs can include, so a new folder is covered without editing this test.
test("no two compiled pod sources share a file name", () => {
  const seen = new Map();
  const walk = (dir) => {
    for (const e of readdirSync(dir, { withFileTypes: true })) {
      const path = join(dir, e.name);
      if (e.isDirectory()) walk(path);
      else if (/\.(swift|m|mm)$/.test(e.name)) seen.set(e.name, [...(seen.get(e.name) ?? []), path.slice(pkg.length + 1)]);
    }
  };
  walk(p("ios"));
  assert.ok(seen.size > 20, `found ${seen.size} compiled sources under ios/ -- is the tree there?`);
  const dupes = [...seen].filter(([, paths]) => paths.length > 1);
  assert.deepEqual(dupes, [], `duplicate file names in one pod target: ${JSON.stringify(dupes)}`);
});
