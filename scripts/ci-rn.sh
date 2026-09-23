#!/usr/bin/env bash
# React Native (packages/react-native + its example app) in CI, so a change to the
# shared native code can't break it unnoticed -- it did on 2026-09-20, and nothing
# in CI compiled RN at the time.
#
#   scripts/ci-rn.sh --js       package build + JS tests (no native toolchain needed)
#   scripts/ci-rn.sh --android  the example app, in both voice-vendor configurations
#   scripts/ci-rn.sh --ios      the pod (see below), not the example app
#
# Flags combine. The android and ios halves need this package's copied engine
# artifacts: run `COPY_ONLY=1 packages/react-native/build.sh` after
# packages/{ios,android}/build.sh, which the native CI jobs already do.
set -euo pipefail
cd "$(dirname "$0")/.."

JS=0
ANDROID=0
IOS=0
for arg in "$@"; do
  case "$arg" in
    --js) JS=1 ;;
    --android) ANDROID=1 ;;
    --ios) IOS=1 ;;
    *) echo "usage: $0 [--js] [--android] [--ios]" >&2; exit 2 ;;
  esac
done
[ $((JS + ANDROID + IOS)) -gt 0 ] || { echo "usage: $0 [--js] [--android] [--ios]" >&2; exit 2; }

step() { printf '\n==> %s\n' "$*"; }

if [ "$JS" = 1 ]; then
  step "react-native: npm ci";        (cd packages/react-native && npm ci --ignore-scripts)
  step "react-native: npm run build"; (cd packages/react-native && npm run build)
  step "react-native: npm test";      (cd packages/react-native && npm test)
fi

# This package compiles *copies* of the engine bindings and of the shared native code
# (packages/ios/Sources/Sinua, packages/android/view, the voice sources). A stale
# copy is exactly how an RN break hides, so every run refreshes it for the platform it
# builds -- the copy needs that platform's build.sh output to exist already.
refresh_copies() {
  local only="$1"
  local artifact="packages/react-native/android/src/main/jniLibs"
  [ "$only" = ios ] && artifact="packages/react-native/core_engineFFI.xcframework"
  local source_dir="packages/android/src/main/jniLibs"
  [ "$only" = ios ] && source_dir="packages/ios/core_engineFFI.xcframework"
  [ -e "$source_dir" ] || {
    echo "$source_dir is missing; run packages/$([ "$only" = ios ] && echo ios || echo android)/build.sh first" >&2
    exit 1
  }
  # build.sh ends with the package's own `npm run build` (tsc), which needs the
  # package's dependencies. The native jobs don't run --js, so on a clean runner they
  # were missing and tsc exited 2 -- with its errors sent to /dev/null, which is why
  # the first Actions run (2026-09-21) showed a bare "exit code 2". Install if absent,
  # and show build.sh's output when it fails instead of discarding it.
  [ -d packages/react-native/node_modules ] || {
    step "react-native: npm ci (the package's own dependencies, for build.sh's tsc)"
    (cd packages/react-native && npm ci --ignore-scripts)
  }
  step "react-native: refresh the copied $only artifacts"
  local log
  log="$(mktemp)"
  (cd packages/react-native && COPY_ONLY=1 ONLY="$only" ./build.sh > "$log" 2>&1) || {
    cat "$log" >&2
    exit 1
  }
  [ -e "$artifact" ] || { echo "the copy did not produce $artifact" >&2; exit 1; }
}

if [ "$ANDROID" = 1 ]; then
  refresh_copies android
  # Gradle wants ANDROID_HOME; CI images set it, a local Mac usually has it here
  # (the same default as scripts/ci-local.sh).
  : "${ANDROID_HOME:=$HOME/Library/Android/sdk}"; export ANDROID_HOME
  step "react-native: example npm ci"; (cd packages/react-native/example && npm ci --ignore-scripts)
  # Twice: the default vendors, then all four. The second proves the opt-in
  # source sets and dependencies still compile (docs/fx-view.md, *React Native*).
  step "react-native: example Android (default vendors)"
  (cd packages/react-native/example/android && ./gradlew --no-daemon -q :app:assembleDebug)
  step "react-native: example Android (livekit + openai)"
  (cd packages/react-native/example/android && ./gradlew --no-daemon -q :app:assembleDebug \
    -Psinua.voiceVendors=gemini,elevenlabs,livekit,openai)
fi

if [ "$IOS" = 1 ]; then
  refresh_copies ios
  step "react-native: example npm ci"; (cd packages/react-native/example && npm ci --ignore-scripts)
  step "react-native: pod install";    (cd packages/react-native/example/ios && pod install)
  # The pod (our Swift/ObjC + the Fabric component), not the example *app*: linking the
  # app currently fails inside React Native's own prebuilt Hermes (undefined
  # `facebook::hermes::cdp::CDPDebugAPI::*` from libReact-hermes.a), with or without this
  # package. studio-ui-ux owns that one; building the pod still covers everything we ship.
  step "react-native: xcodebuild the SinuaCore pod (iOS Simulator)"
  (cd packages/react-native/example/ios && xcodebuild build -quiet \
    -workspace SinuaExample.xcworkspace -scheme SinuaCore -configuration Debug \
    -destination 'generic/platform=iOS Simulator' \
    -derivedDataPath "${RN_DERIVED_DATA:-build/rn-derived-data}")
fi

echo
echo "react-native CI: ok"
