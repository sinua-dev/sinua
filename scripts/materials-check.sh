#!/usr/bin/env bash
# The materials/geometry cross-language check: does the Web, iOS and Android
# painter draw the *same frame*? This is the pixel half of that claim; the
# colour half is spec/paint-ink-vectors.json.
#
#   scripts/materials-check.sh                 # collect from both natives, compare
#   scripts/materials-check.sh --ios-only      # Web vs iOS
#   scripts/materials-check.sh --android-only  # Web vs Android
#   scripts/materials-check.sh --render-ios    # also drive MaterialsRenderTests first
#   scripts/materials-check.sh --compare-only  # $MATERIALS_OUT is already staged (CI)
#
# Sources of the native renders, in order of preference:
#   iOS      $IOS_XCRESULT, else out/materials/R.xcresult from --render-ios
#   Android  $ANDROID_LOGCAT (a file or a directory to search), else
#            packages/android/view/build/outputs/androidTest-results
# In CI these are downloaded artifacts; locally they are whatever the last
# native test run left behind.
#
# Both native render tests already run on every CI push -- MaterialsRenderTests
# inside -only-testing:SinuaTests, renderMaterialsGoldenCases inside
# :sinua-view:connectedDebugAndroidTest -- so this script collects work that was
# already being done and thrown away. It never boots an emulator and never
# rebuilds the xcframework (packages/ios/build.sh `rm -rf`s it, and three other
# areas share this tree).
#
# Needs Playwright + a Chromium for the Web render: $PLAYWRIGHT / $CHROME, the
# same variables gen-ink-vectors.mjs and gen-voice-golden.mjs take.
set -euo pipefail
cd "$(dirname "$0")/.."

MAT=packages/web/scripts/materials
# Absolute, because two steps below run from another directory (frames.mjs
# needs `@sinua/core` resolvable from its own, xcodebuild from packages/ios).
OUT="${MATERIALS_OUT:-$PWD/out/materials}"
case "$OUT" in /*) ;; *) OUT="$PWD/$OUT" ;; esac
PLATFORMS=ios,android
RENDER_IOS=0
COMPARE_ONLY=0

while [ $# -gt 0 ]; do
  case "$1" in
    --ios-only) PLATFORMS=ios ;;
    --android-only) PLATFORMS=android ;;
    --render-ios) RENDER_IOS=1 ;;
    --compare-only) COMPARE_ONLY=1 ;;
    -h|--help) sed -n '2,20p' "$0"; exit 0 ;;
    *) echo "materials-check: unknown argument $1" >&2; exit 2 ;;
  esac
  shift
done

step() { printf '\n==> %s\n' "$*"; }
has() { case ",$PLATFORMS," in *",$1,"*) return 0 ;; *) return 1 ;; esac; }

if [ "$COMPARE_ONLY" = 0 ]; then
  mkdir -p "$OUT/png"
  rm -f "$OUT/png"/*.png
  step "materials: the Web's frames (spec/sinua-golden.json + SYNTHETIC)"
  (cd "$MAT" && node --experimental-wasm-modules frames.mjs "$OUT/frames.json")
else
  # CI: the frames and both render sets arrive as artifacts from the web, ios
  # and android jobs, so nothing here needs wasm, a simulator or an emulator.
  [ -f "$OUT/frames.json" ] || { echo "materials: --compare-only needs $OUT/frames.json" >&2; exit 1; }
  [ -d "$OUT/png" ] || { echo "materials: --compare-only needs $OUT/png" >&2; exit 1; }
fi
EXPECT=$(node -e 'console.log(JSON.parse(require("fs").readFileSync(process.argv[1])).length)' "$OUT/frames.json")
echo "materials: $EXPECT case(s) expected from each platform"

if [ "$COMPARE_ONLY" = 0 ] && has ios; then
  if [ "$RENDER_IOS" = 1 ]; then
    # -only-testing is mandatory here, not tidiness: this simulator is shared
    # between sessions, and an unfiltered run once hung in `simctl diagnose`
    # (docs/testing.md). Reuse a booted iPhone; never shut one down or erase it.
    udid="${IOS_SIM_UDID:-$(xcrun simctl list devices | awk '/\(Booted\)/ {print $(NF-1); exit}' | tr -d '()')}"
    [ -n "$udid" ] || { echo "materials: no booted iPhone simulator; set IOS_SIM_UDID" >&2; exit 1; }
    step "materials: iOS renders on $udid (MaterialsRenderTests only)"
    rm -rf "$OUT/R.xcresult"
    (cd packages/ios && xcodebuild test \
      -scheme Sinua-Package \
      -destination "id=$udid" \
      -only-testing:SinuaTests/MaterialsRenderTests \
      -collect-test-diagnostics never \
      -resultBundlePath "$OUT/R.xcresult" >/dev/null)
    IOS_XCRESULT="$OUT/R.xcresult"
  fi
  step "materials: collect iOS renders"
  node "$MAT/collect-ios.mjs" "${IOS_XCRESULT:-$OUT/R.xcresult}" "$OUT/png"
fi

if [ "$COMPARE_ONLY" = 0 ] && has android; then
  step "materials: collect Android renders"
  node "$MAT/collect-android.mjs" \
    "${ANDROID_LOGCAT:-packages/android/view/build/outputs/androidTest-results}" "$OUT/png"
fi

step "materials: compare in Chrome (docs/fx-view.md tolerance)"
node "$MAT/compare.cjs" "$OUT/frames.json" "$OUT/png" "$OUT/sheet.png" \
  --platforms "$PLATFORMS" --expect "$EXPECT"
echo "materials: contact sheet -> $OUT/sheet.png"
