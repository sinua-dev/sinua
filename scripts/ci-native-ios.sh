#!/usr/bin/env bash
# iOS leg of CI (.github/workflows/ci.yml job `ios`, and `scripts/ci-local.sh
# --native`): rebuild the xcframework + Swift bindings, run the four XCTest
# targets on an iOS Simulator (SinuaGeminiLiveTests uses in-process
# localhost fakes -- no network, no audio), then compile the separate LiveKit
# glue package (packages/ios-livekit: SwiftPM, pulls client-sdk-swift + its
# WebRTC binary). One script for both, so they can't drift.
#
# $SPM_CACHE_DIR (optional): where SwiftPM clones packages for the LiveKit
# build -- CI points it at a cached directory; unset locally = Xcode's default.
#
# Simulator choice: $IOS_SIM_UDID if set, else an already-booted iPhone (the
# local machine's simulator is shared between sessions -- reuse it, never shut
# it down or erase it), else the first available iPhone on the newest runtime.
# Always -only-testing: an unfiltered run once hung in `simctl diagnose`
# (docs/testing.md), and -collect-test-diagnostics never keeps a failure from
# triggering the same sysdiagnose.
set -euo pipefail
cd "$(dirname "$0")/.."

step() { printf '\n==> %s\n' "$*"; }

# First, because it takes a second or two and failing on formatting before a
# simulator boot beats failing after one. `swift format` ships with the Swift
# toolchain, so this adds no dependency to a runner. The runner excludes
# generated Swift (the codegen's Sources/Sinua/Generated, UniFFI's
# core_engine.swift): reformatting either would be undone by the next generator
# run and would break `generate.mjs --check`.
step "ios: swift format lint (packages/ios*, generated Swift excluded)"; scripts/lint-swift.sh

# The views must not pull in the microphone code: an app that only draws would
# otherwise link LocalMicVoiceSource / AVPcmAudioDevice (release decision 0.4). Walks the `Sinua` target's dependencies transitively, so a
# SinuaVoice dependency added anywhere below it fails too.
step "ios: the Sinua target does not depend on SinuaVoice (transitively)"
(cd packages/ios && swift package dump-package) | python3 -c '
import json, sys
targets = {t["name"]: t for t in json.load(sys.stdin)["targets"]}
def deps(t):
    for d in targets[t]["dependencies"]:
        kind, args = next(iter(d.items()))
        yield args[0] if isinstance(args[0], str) else None
seen, todo = set(), ["Sinua"]
while todo:
    for d in deps(todo.pop()):
        if d in targets and d not in seen:
            seen.add(d); todo.append(d)
if "SinuaVoice" in seen:
    sys.exit("Sinua reaches SinuaVoice (via " + ", ".join(sorted(seen)) + "): the views would link the microphone code")
print("Sinua depends on: " + ", ".join(sorted(seen)))
'

step "ios: packages/ios/build.sh (bindings + xcframework)"
packages/ios/build.sh

udid="${IOS_SIM_UDID:-}"
if [ -z "$udid" ]; then
  udid="$(xcrun simctl list devices available -j | python3 -c '
import json, sys
runtimes = json.load(sys.stdin)["devices"]
def ver(rt):
    tail = rt.rsplit(".", 1)[-1]            # ...SimRuntime.iOS-18-5
    return [int(p) for p in tail.split("-")[1:] if p.isdigit()]
ios = sorted((rt for rt in runtimes if ".iOS-" in rt), key=ver, reverse=True)
phones = [d for rt in ios for d in runtimes[rt] if d["name"].startswith("iPhone")]
booted = [d for d in phones if d["state"] == "Booted"]
pick = (booted or phones or [None])[0]
print(pick["udid"] if pick else "")
')"
fi
[ -n "$udid" ] || { echo "no available iPhone simulator" >&2; exit 1; }
step "ios: simulator $udid"
xcrun simctl list devices | grep "$udid" || true

# -resultBundlePath keeps the run's attachments, which is the only reason this
# job can feed the materials check: SinuaTests already includes
# MaterialsRenderTests, so the 42 renders are produced either way -- until now
# they were produced and thrown away. xcodebuild refuses an existing bundle
# path, hence the rm; the target is this script's own output directory.
MATERIALS_RESULT="${MATERIALS_RESULT:-$PWD/out/materials/R.xcresult}"
rm -rf "$MATERIALS_RESULT"
mkdir -p "$(dirname "$MATERIALS_RESULT")"

step "ios: xcodebuild test (CoreEngineTests, SinuaVoiceTests, SinuaViewTests, SinuaGeminiLiveTests, SinuaElevenLabsTests)"
(cd packages/ios && xcodebuild test \
  -scheme Sinua-Package \
  -destination "id=$udid" \
  -only-testing:CoreEngineTests \
  -only-testing:SinuaVoiceTests \
  -only-testing:SinuaTests \
  -only-testing:SinuaGeminiLiveTests \
  -only-testing:SinuaElevenLabsTests \
  -collect-test-diagnostics never \
  -resultBundlePath "$MATERIALS_RESULT")

step "ios: xcodebuild build SinuaLiveKit (compile only, no tests in that package)"
spm_args=()
[ -n "${SPM_CACHE_DIR:-}" ] && spm_args=(-clonedSourcePackagesDirPath "$SPM_CACHE_DIR")
(cd packages/ios-livekit && xcodebuild build \
  -scheme SinuaLiveKit \
  -destination 'generic/platform=iOS Simulator' \
  ${spm_args[@]+"${spm_args[@]}"})

step "ios: xcodebuild build SinuaOpenAI (compile only: its WebRTC call would open real audio)"
(cd packages/ios-openai && xcodebuild build \
  -scheme SinuaOpenAI \
  -destination 'generic/platform=iOS Simulator' \
  ${spm_args[@]+"${spm_args[@]}"})

step "ios: xcodebuild build the docs site's Swift samples (apps/site/snippets, compile only)"
scripts/docs/check-code-snippets.sh --native-swift
