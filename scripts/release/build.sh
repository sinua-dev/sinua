#!/usr/bin/env bash
# Every artefact a release publishes, built into out/release/<version>/ -- nothing is
# uploaded from here (.github/workflows/release.yml does that, per target, only when
# its credentials exist). docs/publishing.md, *How to release*.
#
#   scripts/release/build.sh              # what a release would publish
#   scripts/release/build.sh --dry-run    # the same, plus the local SwiftPM trees a
#                                         # scratch consumer can build against
#   scripts/release/build.sh --publishable
#                                         # refuse placeholders (the real release)
#   scripts/release/build.sh --only npm|maven|swift
#                                         # one target (release.yml builds each on its own runner)
#
# Prerequisites: the engine builds (packages/ios/build.sh, packages/android/build.sh,
# packages/core `npm run build`) -- CI runs them before this, as the release job does.
#
# Out:
#   npm/      @sinua/core, @sinua/web, @sinua/voice tarballs
#   maven/    a Maven-layout staging repo, every module; signed when ORG_GRADLE_PROJECT_signingKey is set
#   swift/    core_engineFFI.xcframework.zip + checksum, and the three distribution trees
#   manifest.json  every file with its sha256
set -euo pipefail
cd "$(dirname "$0")/../.."
ROOT="$PWD"

DRY=0; PUBLISHABLE=""; ONLY=""
while [ $# -gt 0 ]; do
  case "$1" in
    --dry-run) DRY=1 ;;
    --publishable) PUBLISHABLE="--publishable" ;;
    --only) ONLY="$2"; shift ;;
    *) echo "usage: $0 [--dry-run] [--publishable] [--only npm|maven|swift]" >&2; exit 2 ;;
  esac
  shift
done
want() { [ -z "$ONLY" ] || [ "$ONLY" = "$1" ]; }

step() { printf '\n==> %s\n' "$*"; }

node scripts/release/set-version.mjs --check
VERSION="$(tr -d '[:space:]' < VERSION)"
OUT="$ROOT/out/release/$VERSION"
if [ -z "$ONLY" ]; then rm -rf "$OUT"; else rm -rf "$OUT/$ONLY" "$OUT/swift-local"; fi
mkdir -p "$OUT"
step "release $VERSION -> out/release/$VERSION"

if want npm; then
step "npm: pack @sinua/core, @sinua/web, @sinua/voice"
node scripts/release/npm-pack.mjs "$OUT/npm" $PUBLISHABLE
fi

if want maven; then
step "maven: stage every module"
if [ -z "${ORG_GRADLE_PROJECT_signingKey:-}" ]; then
  echo "   (no ORG_GRADLE_PROJECT_signingKey: staged UNSIGNED -- fine for a dry run, refused by Maven Central)"
  [ -n "$PUBLISHABLE" ] && { echo "a publishable build needs a signing key" >&2; exit 1; }
fi
(cd packages/android && ./gradlew --no-daemon -q publishAllPublicationsToStagingRepository -Psinua.stagingRepo="$OUT/maven")
echo "   $(find "$OUT/maven" -name '*.pom' | wc -l | tr -d ' ') modules staged"
fi

if want swift; then
step "swift: zip the xcframework, checksum, distribution trees"
mkdir -p "$OUT/swift"
ZIP="$OUT/swift/core_engineFFI.xcframework.zip"
(cd packages/ios && ditto -c -k --sequesterRsrc --keepParent core_engineFFI.xcframework "$ZIP")
CHECKSUM="$(cd "$OUT/swift" && swift package compute-checksum core_engineFFI.xcframework.zip)"
echo "$CHECKSUM" > "$OUT/swift/checksum.txt"
node scripts/release/swift-dist.mjs "$OUT/swift" "$VERSION" "$CHECKSUM"
if [ "$DRY" = 1 ]; then
  node scripts/release/swift-dist.mjs "$OUT/swift-local" "$VERSION" "$CHECKSUM" --local "$ZIP"
fi
fi

step "manifest"
node -e '
  const fs = require("fs"), path = require("path"), crypto = require("crypto");
  const out = process.argv[1], files = [];
  const walk = (d) => { for (const e of fs.readdirSync(d, { withFileTypes: true })) {
    const p = path.join(d, e.name);
    if (e.isDirectory()) { if (e.name !== "swift-local") walk(p); }
    else if (p !== path.join(out, "manifest.json")) files.push({ path: path.relative(out, p), sha256: crypto.createHash("sha256").update(fs.readFileSync(p)).digest("hex"), bytes: fs.statSync(p).size });
  } };
  walk(out);
  fs.writeFileSync(path.join(out, "manifest.json"), JSON.stringify({ version: process.argv[2], files }, null, 1) + "\n");
  console.log(`   ${files.length} files`);
' "$OUT" "$VERSION"

step "done: out/release/$VERSION"
