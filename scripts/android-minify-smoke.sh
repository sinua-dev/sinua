#!/usr/bin/env bash
# The minified-release smoke: build apps/android-minify-smoke with R8 on (as an app
# ships to Play), install it on the connected device/emulator, launch it and wait for
# the engine call and the first drawn frame. Fails on a crash, an UnsatisfiedLinkError
# (the 0.1.0-beta.4 failure: JNA stripped by R8) or a timeout. The app is uninstalled
# afterwards. Uses whatever device ANDROID_SERIAL / adb picks.
set -euo pipefail
cd "$(dirname "$0")/.."

: "${ANDROID_HOME:=$HOME/Library/Android/sdk}"; export ANDROID_HOME
ADB="${ADB:-$ANDROID_HOME/platform-tools/adb}"
PKG=dev.sinua.minifysmoke
TIMEOUT="${SMOKE_TIMEOUT:-60}"

echo "==> android-minify-smoke: assembleRelease (R8 on)"
(cd packages/android && ./gradlew --no-daemon :android-minify-smoke:assembleRelease)
APK=apps/android-minify-smoke/build/outputs/apk/release/android-minify-smoke-release.apk

"$ADB" install -r "$APK" >/dev/null
trap '"$ADB" uninstall "$PKG" >/dev/null 2>&1 || true' EXIT
"$ADB" logcat -c
"$ADB" shell am start -W -n "$PKG/.MainActivity" >/dev/null

for _ in $(seq 1 "$TIMEOUT"); do
  log="$("$ADB" logcat -d -v brief SinuaSmoke:I AndroidRuntime:E '*:S')"
  if grep -q -E "FATAL EXCEPTION|UnsatisfiedLinkError" <<<"$log"; then
    echo "$log" >&2
    echo "android-minify-smoke: FAILED (crash in the minified release build)" >&2
    exit 1
  fi
  if grep -q "engine ok=true" <<<"$log" && grep -q -E "SinuaSmoke.*: frame *$" <<<"$log"; then
    echo "android-minify-smoke: ok (engine call + first frame in a minified release build)"
    exit 0
  fi
  sleep 1
done
"$ADB" logcat -d -v brief SinuaSmoke:I AndroidRuntime:E '*:S' >&2
echo "android-minify-smoke: FAILED (no engine/frame log within ${TIMEOUT}s)" >&2
exit 1
