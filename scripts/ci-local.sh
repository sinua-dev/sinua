#!/usr/bin/env bash
# Local counterpart to .github/workflows/ci.yml -- the same commands in the same
# order, fail-fast. Uses `npm ci`, so it reinstalls node_modules from the
# lockfiles exactly as CI does. Keep the two in sync when either changes.
#
# It is a *subset* by default, and says so at the end rather than claiming to be
# a full mirror (it used to print "CI mirror: all green" after skipping three CI
# jobs).
#
#   scripts/ci-local.sh           the rust + web jobs, including the React
#                                 Native JS leg, which CI also runs
#                                 unconditionally. The fast default.
#   scripts/ci-local.sh --all     every job below: the full mirror.
#   scripts/ci-local.sh --list    print what would run, run nothing, exit.
#   scripts/ci-local.sh --native  also the ios + android jobs
#                                 (scripts/ci-native-{ios,android}.sh).
#   scripts/ci-local.sh --examples  also apps/examples/web-component: the five
#                                 <sinua-view> framework apps (build, typecheck,
#                                 headless smoke). CI runs these inside its web
#                                 job; they are opt-in here because of their own
#                                 541-package install.
#   scripts/ci-local.sh --site    also the docs site (apps/site: its own npm ci,
#                                 the static-export `next build`, headless smoke).
#   scripts/ci-local.sh --rn      with --native, also the React Native *native*
#                                 halves: the iOS pod and the example app on
#                                 Android in both vendor configurations. The JS
#                                 half is in the default run.
#
# Flags combine. The docs checks (generated partials, snippets) are in the
# default run.
#
# Two differences from CI that no flag removes, both environmental:
#   * CI pins wasm-pack (ci.yml, taiki-e/install-action). Here we use whatever
#     is on PATH and warn when it differs.
#   * CI installs Playwright's Chromium in the jobs that need it. Here the site
#     and examples smokes use the browser you already have, or $SINUA_CHROME.
set -euo pipefail
cd "$(dirname "$0")/.."

# The wasm-pack version .github/workflows/ci.yml pins.
CI_WASM_PACK=0.13.1

NATIVE=0
EXAMPLES=0
SITE=0
RN=0
LIST=0
for arg in "$@"; do
  case "$arg" in
    --native) NATIVE=1 ;;
    --examples) EXAMPLES=1 ;;
    --site) SITE=1 ;;
    --rn) RN=1 ;;
    --all) NATIVE=1; EXAMPLES=1; SITE=1; RN=1 ;;
    --list) LIST=1 ;;
    *) echo "usage: $0 [--all] [--list] [--native] [--examples] [--site] [--rn]" >&2; exit 2 ;;
  esac
done

# What this invocation covers, and what it leaves out -- printed by --list and
# again at the end, so a partial run is never mistaken for a full one.
skipped() {
  local missing=()
  [ "$NATIVE" = 1 ]   || missing+=("native legs (ios, android)")
  [ "$SITE" = 1 ]     || missing+=("docs site")
  [ "$EXAMPLES" = 1 ] || missing+=("framework examples")
  [ "$NATIVE$RN" = 11 ] || missing+=("react-native native halves")
  # "${a[*]}" joins on IFS's first character only, so build the ", " by hand.
  [ ${#missing[@]} -eq 0 ] || { local out="${missing[0]}" i; for i in "${missing[@]:1}"; do out="$out, $i"; done; echo "$out"; }
}

if [ "$LIST" = 1 ]; then
  echo "scripts/ci-local.sh would run:"
  echo "  rust:  fmt, clippy, test --workspace"
  echo "  web:   codegen, packages/{core,web,snippets,voice,react-native}, docs checks"
  echo "  web:   react-native JS leg (scripts/ci-rn.sh --js)"
  [ "$NATIVE" = 1 ]   && echo "  ios:   scripts/ci-native-ios.sh"
  [ "$NATIVE$RN" = 11 ] && echo "  ios:   scripts/ci-rn.sh --ios (the pod)"
  [ "$NATIVE" = 1 ]   && echo "  android: scripts/ci-native-android.sh build + connected"
  [ "$NATIVE$RN" = 11 ] && echo "  android: scripts/ci-rn.sh --android (the example app)"
  [ "$EXAMPLES" = 1 ] && echo "  examples: apps/examples/web-component build + typecheck + smoke"
  [ "$SITE" = 1 ]     && echo "  site:  apps/site build + headless smoke"
  not_run="$(skipped)"
  [ -n "$not_run" ] && echo "not run: $not_run  (--all runs everything)"
  exit 0
fi

step() { printf '\n==> %s\n' "$*"; }

step "rust: toolchain from rust-toolchain.toml"
rustup show active-toolchain || rustup toolchain install
rustup component add rustfmt clippy
rustup target add wasm32-unknown-unknown

step "rust: cargo fmt";    cargo fmt --all --check
step "rust: cargo clippy"; cargo clippy --workspace --all-targets -- -D warnings
step "rust: cargo test";   cargo test --workspace

step "web: wasm-pack version"; wasm-pack --version
# CI pins this; a different local one means you are not testing what CI tests.
have_wasm_pack="$(wasm-pack --version | awk '{print $2}')"
[ "$have_wasm_pack" = "$CI_WASM_PACK" ] || \
  echo "    note: CI pins wasm-pack $CI_WASM_PACK, this machine has $have_wasm_pack" >&2
step "web: node version";      node --version

step "release: every package at VERSION"; node scripts/release/set-version.mjs --check
step "codegen: typed components up to date"; node scripts/codegen/generate.mjs --check
step "codegen: public API snapshot";        node scripts/codegen/api-snapshot.mjs
step "codegen: tests";                      node --test 'scripts/codegen/test/*.test.mjs'

step "packages/core: npm ci";        (cd packages/core && npm ci)
step "packages/core: npm run build"; (cd packages/core && npm run build)
step "packages/core: npm test";      (cd packages/core && npm test)

# The docs site's generated tables and code samples (apps/site): partials match spec/parameters.json + the engine's messages,
# and every snippet resolves / type-checks. Needs packages/core's dist.
step "docs: generated partials up to date"; node scripts/docs/gen.mjs --check
step "docs: snippets resolve and type-check"; node scripts/docs/check-snippets.mjs

step "packages/web: npm ci";        (cd packages/web && npm ci)
step "packages/web: npm run build"; (cd packages/web && npm run build)
step "packages/web: npm test";      (cd packages/web && npm test)
# After packages/web: the bundler check also bundles @sinua/web/element(s) (web dist/).
step "packages/core: bundler check"; (cd packages/core/bundler-check && npm ci && npm test)



# The export text the Studio and the docs site hand out (no engine dependency);
# its tests are the byte-for-byte lock the native ports are diffed against.
step "packages/snippets: npm ci";        (cd packages/snippets && npm ci)
step "packages/snippets: npm run build"; (cd packages/snippets && npm run build)
step "packages/snippets: npm test";      (cd packages/snippets && npm test)

step "packages/voice: npm ci";      (cd packages/voice && npm ci)
step "packages/voice: npm run build"; (cd packages/voice && npm run build)
step "packages/voice: npm test";      (cd packages/voice && npm test)
step "packages/react-native: npm ci (types for the docs' RN samples)"; (cd packages/react-native && npm ci --ignore-scripts)
step "docs: code samples (Web + RN type-check, install names)"; scripts/docs/check-code-snippets.sh --web
step "scripts/test: the install-snippet checker fails when it checked nothing"; node --test 'scripts/test/*.test.mjs'

# CI runs this unconditionally in its web job (a change to the shared native code
# broke RN unnoticed on 2026-09-20), so it is in the default run here too. The
# JS half needs no native toolchain; the app builds do, so they follow --native.
step "react-native: package build + JS tests"; scripts/ci-rn.sh --js

if [ "$NATIVE" = 1 ]; then
  step "ios"; scripts/ci-native-ios.sh
  if [ "$RN" = 1 ]; then step "react-native: the iOS pod"; scripts/ci-rn.sh --ios; fi

  # Android: Gradle wants ANDROID_HOME; the emulator lives in the homebrew
  # command-line-tools root.
  : "${ANDROID_HOME:=$HOME/Library/Android/sdk}"; export ANDROID_HOME
  : "${EMULATOR_SDK_ROOT:=/opt/homebrew/share/android-commandlinetools}"
  : "${ANDROID_AVD:=Nutp_Test}"
  ADB="$ANDROID_HOME/platform-tools/adb"

  step "android: build + JVM tests"; scripts/ci-native-android.sh build
  if [ "$RN" = 1 ]; then step "react-native: the example app on Android"; scripts/ci-rn.sh --android; fi

  # Reuse a device that's already up; otherwise boot the AVD headless and kill
  # only the emulator pid started here (the AVD is shared between sessions).
  EMU_PID=""
  if ! "$ADB" devices | awk 'NR>1 && $2=="device"' | grep -q .; then
    step "android: booting $ANDROID_AVD (headless)"
    # Both SDK variables point at the emulator's root: the emulator reads
    # ANDROID_HOME first, and the Gradle one exported above has no system
    # image for this AVD ("Broken AVD system path", seen 2026-09-19).
    ANDROID_HOME="$EMULATOR_SDK_ROOT" ANDROID_SDK_ROOT="$EMULATOR_SDK_ROOT" \
      "$EMULATOR_SDK_ROOT/emulator/emulator" \
      -avd "$ANDROID_AVD" -no-snapshot-save -no-window -no-audio -no-boot-anim \
      >/dev/null 2>&1 &
    EMU_PID=$!
    trap '[ -n "$EMU_PID" ] && kill "$EMU_PID" 2>/dev/null || true' EXIT
    # Bounded poll, never a bare `adb wait-for-device` (it hangs forever if
    # the emulator dies): up to 6 minutes.
    booted=0
    for _ in $(seq 1 180); do
      [ "$("$ADB" shell getprop sys.boot_completed 2>/dev/null | tr -d '\r')" = 1 ] && { booted=1; break; }
      kill -0 "$EMU_PID" 2>/dev/null || { echo "emulator exited during boot" >&2; exit 1; }
      sleep 2
    done
    [ "$booted" = 1 ] || { echo "emulator did not boot in 6 minutes" >&2; exit 1; }
  fi
  step "android: instrumented tests"; scripts/ci-native-android.sh connected
fi

if [ "$EXAMPLES" = 1 ]; then
  # Opt-in: its own install (Vue/Svelte/Solid/Angular), outside the other jobs.
  # packages/web's dist must be built (the web job above does it).
  step "examples: <sinua-view> in HTML/Vue/Svelte/Solid/Angular"
  ( cd apps/examples/web-component && npm ci && npm run build && npm run typecheck && npm run smoke )
fi

if [ "$SITE" = 1 ]; then
  # Opt-in: the docs site's own install (Next.js + Fumadocs), outside the other
  # jobs. The static export compiles every MDX page and <include>, so it
  # catches content errors (frontmatter YAML, broken includes) the default
  # docs checks don't. Run it on a copy, not beside a running `next dev`
  # (both use apps/site/.next).
  step "site: docs static export"
  ( cd apps/site && npm ci && NEXT_TELEMETRY_DISABLED=1 npm run build )
  # Every exported page in headless Chromium, served like a plain static host:
  # fails on console errors (e.g. React #418 hydration) and a dead search.
  step "site: headless check of the export"
  ( cd apps/site && npm run smoke )
fi


# The repo-wide TypeScript gate, last: type-aware rules resolve imports through
# each linted area's node_modules and dist/, so everything it lints must be
# installed first (CI ran it too early on 2026-09-21 and got 3,348 errors).
# apps/site is installed only with --site; without it the site is left out of
# the lint rather than linted against missing types, and the run says so.
if [ "$SITE" = 1 ]; then
  step "lint: TypeScript (repo-wide)"; npm ci && npm run lint
else
  step "lint: TypeScript (repo-wide except apps/site, which needs --site)"
  npm ci && npm run lint -- --ignore-pattern 'apps/site/**'
fi

not_run="$(skipped)"
if [ -n "$not_run" ]; then
  step "green -- but this was a subset of CI. Not run: $not_run"
  echo "    scripts/ci-local.sh --all runs every job; --list shows the plan without running it."
else
  step "green -- every CI job ran locally"
fi
