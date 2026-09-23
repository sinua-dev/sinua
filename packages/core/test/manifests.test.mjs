// What a registry sees. Every `@sinua/*` manifest was missing `author`,
// `repository`, `homepage`, `bugs` and `keywords` -- and `repository` is not
// cosmetic: npm provenance requires a public `repository` field that matches,
// case-sensitively, where the package is published from (docs/publishing.md).
//
// Those fields point at the public repository (sinua-dev/sinua). A placeholder
// (`<ANYTHING>`) is tolerated while a package is `private`, and a shipped one is
// worse than a missing field -- so the rule here is conditional: **placeholders
// are fine until `private` comes off, and a failure at that moment is the
// point.** scripts/release/npm-pack.mjs --publishable refuses them too.
//
// It also cross-checks `author` against NOTICE, because those two drifted apart
// once already (publishing.md); families asserts the same thing on the Rust side.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import { dirname, join } from "node:path";

const packages = join(dirname(fileURLToPath(import.meta.url)), "../..");
const repoRoot = join(packages, "..");

const PUBLISHABLE = ["core", "web", "voice", "snippets", "react-native"];
const REQUIRED = ["name", "version", "description", "license", "author", "repository", "homepage", "bugs", "keywords", "engines"];

/** `<ANYTHING>` -- the repo's one placeholder spelling. */
const PLACEHOLDER = /<[A-Z][A-Z0-9_]*>/;

const manifest = (pkg) => JSON.parse(readFileSync(join(packages, pkg, "package.json"), "utf8"));

/** The authors string, from the file that is the licence's own record of it. */
function noticeAuthor() {
  const notice = readFileSync(join(repoRoot, "NOTICE"), "utf8");
  const m = /Copyright\s+\d{4}\s+(.+)/.exec(notice);
  assert.ok(m, "NOTICE has no `Copyright <year> <authors>` line");
  return m[1].trim();
}

for (const pkg of PUBLISHABLE) {
  test(`${pkg}: the manifest carries what a registry needs`, (t) => {
    if (!existsSync(join(packages, pkg, "package.json"))) return t.skip(`packages/${pkg} is not present`);
    const d = manifest(pkg);
    const missing = REQUIRED.filter((k) => d[k] === undefined);
    assert.deepEqual(missing, [], `packages/${pkg}/package.json is missing ${missing.join(", ")}`);
    assert.ok(Array.isArray(d.keywords) && d.keywords.length > 0, "keywords must be a non-empty array");
    assert.equal(typeof d.repository?.url, "string", "repository must be the object form, so npm can read the URL");
  });

  test(`${pkg}: author matches NOTICE`, (t) => {
    if (!existsSync(join(repoRoot, "NOTICE"))) return t.skip("NOTICE is not present");
    assert.equal(
      manifest(pkg).author,
      noticeAuthor(),
      `packages/${pkg}'s author must be the string NOTICE claims; these drifted apart once already`,
    );
  });

  test(`${pkg}: no placeholder survives into a publishable manifest`, (t) => {
    const d = manifest(pkg);
    const withPlaceholders = Object.entries(d)
      .filter(([, v]) => PLACEHOLDER.test(JSON.stringify(v)))
      .map(([k]) => k);

    if (d.private === true) {
      // Recorded rather than skipped silently, so the state is visible in the
      // test output.
      return t.skip(`private package; unresolved placeholders in: ${withPlaceholders.join(", ") || "none"}`);
    }
    assert.deepEqual(
      withPlaceholders,
      [],
      `packages/${pkg} is publishable but still has placeholders in ${withPlaceholders.join(", ")} -- ` +
        "fill them before dropping `private` (docs/publishing.md's release order)",
    );
  });
}
