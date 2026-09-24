#!/usr/bin/env bash
# Android leg of CI (.github/workflows/ci.yml job `android`, and
# `scripts/ci-local.sh --native`). One script for both, so they can't drift.
#
#   build      rebuild the Kotlin bindings + jniLibs (packages/android/build.sh),
#              then the JVM unit tests (voice golden parity, SinuaView performance,
#              :sinua-gemini's Gemini Live session against localhost fakes)
#              and a compile of :sinua-livekit (the LiveKit glue module; it
#              has no tests of its own, its tracker logic is tested in the root)
#   connected  instrumented tests on the connected device/emulator: both golden
#              sets + FX Spec (root module) and the SinuaView render test
#              (:sinua-view), then the minified-release smoke
#              (scripts/android-minify-smoke.sh: R8 on, consumer rules only).
#
# ANDROID_NDK_HOME (build) and ANDROID_HOME/ANDROID_SDK_ROOT (Gradle) come from
# the environment; the ubuntu runner sets both.
set -euo pipefail
cd "$(dirname "$0")/.."

step() { printf '\n==> %s\n' "$*"; }

case "${1:-}" in
  build)
    # First, for the same reason as the Swift gate in ci-native-ios.sh: failing
    # on formatting before a Gradle build beats failing after one. The runner
    # pins ktlint 1.8.0 and fetches the CLI into .cache/ rather than adding a
    # Gradle plugin -- three areas build packages/android, and a plugin there
    # would change the build for all of them. Generated Kotlin is excluded
    # (**/uniffi/, **/generated/): reformatting it is undone by the next
    # generator run and breaks `generate.mjs --check`.
    step "android: ktlint (packages/android, generated Kotlin excluded)"; scripts/lint-kotlin.sh

    step "android: packages/android/build.sh (bindings + jniLibs)"
    packages/android/build.sh
    step "android: JVM unit tests"
    (cd packages/android && ./gradlew --no-daemon :testDebugUnitTest :sinua-view:testDebugUnitTest :sinua-gemini:testDebugUnitTest :sinua-elevenlabs:testDebugUnitTest :sinua-openai:testDebugUnitTest)
    step "android: :sinua-livekit / :sinua-openai / :site-snippets (the docs' Kotlin + XML samples) assembleDebug"
    (cd packages/android && ./gradlew --no-daemon :sinua-livekit:assembleDebug :sinua-openai:assembleDebug :site-snippets:assembleDebug)
    ;;
  connected)
    # Android 15's cached-app freezer can freeze and kill the test process in the
    # gap between a test finishing (its activity closed, so the process is
    # "cached") and the runner reporting the result -- logcat: "Unable to freeze
    # binder for <pid>", then "exited due to signal 9", and the run ends with
    # "Expected 14 tests, received 9". The tests themselves pass. CI hit it on
    # renderMaterialsGoldenCases twice running (2026-09-21); locally 1 run in 4.
    # The setting takes effect live (dumpsys activity settings: use_freezer=false).
    # CI only: on a developer's machine this is a device-wide setting on
    # whatever device ANDROID_SERIAL / adb picks, so it isn't changed silently.
    if [ -n "${CI:-}" ]; then
      : "${ANDROID_HOME:=$HOME/Library/Android/sdk}"
      "${ADB:-$ANDROID_HOME/platform-tools/adb}" shell settings put global cached_apps_freezer disabled
    fi
    step "android: instrumented tests (root + :sinua-view)"
    (cd packages/android && ./gradlew --no-daemon :connectedDebugAndroidTest :sinua-view:connectedDebugAndroidTest)
    step "android: minified release smoke (R8 on, consumer rules only)"
    scripts/android-minify-smoke.sh
    ;;
  *)
    echo "usage: $0 build|connected" >&2
    exit 2
    ;;
esac
