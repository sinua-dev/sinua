#!/bin/bash
# 16 KB page sizes (Google Play, Android 15+; inbox J1): every 64-bit .so an app gets
# from us -- our libcore_engine.so and the JNA AAR's libjnidispatch.so -- must have all
# PT_LOAD segments aligned to >= 16384. 32-bit ABIs are listed for information only.
#   scripts/check-so-align.sh            (after scripts/ci-native-android.sh build)
set -euo pipefail
cd "$(dirname "$0")/.."
NDK=${ANDROID_NDK_HOME:-/opt/homebrew/share/android-commandlinetools/ndk/27.0.12077973}
READELF=$(ls "$NDK"/toolchains/llvm/prebuilt/*/bin/llvm-readelf | head -1)
JNA=$(grep -ho 'net.java.dev.jna:jna:[0-9.]*' packages/android/build.gradle.kts | cut -d: -f3)
AAR=$(find "$HOME/.gradle/caches" -name "jna-$JNA.aar" 2>/dev/null | head -1)
[ -n "$AAR" ] || { echo "check-so-align: jna-$JNA.aar not in the Gradle cache (run the Android build first)"; exit 1; }
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
unzip -q -o "$AAR" 'jni/*' -d "$TMP/jna"
bad=0
check() { # file label
  local min
  min=$("$READELF" -lW "$1" | awk '$1=="LOAD"{print $NF}' | while read -r a; do echo $((a)); done | sort -n | head -1)
  case "$1" in
    */arm64-v8a/*|*/x86_64/*)
      if [ "$min" -lt 16384 ]; then echo "FAIL $2: p_align $min"; bad=1; else echo "ok   $2: p_align $min"; fi ;;
    *) echo "info $2: p_align $min (32-bit, not a Play rule)" ;;
  esac
}
for f in packages/android/src/main/jniLibs/*/libcore_engine.so "$TMP"/jna/jni/*/libjnidispatch.so; do
  [ -f "$f" ] && check "$f" "${f#"$TMP"/jna/}"
done
exit $bad
