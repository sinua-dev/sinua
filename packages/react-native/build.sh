#!/usr/bin/env bash
# This package has no native build of its own -- it copies packages/ios's
# XCFramework + Swift bindings and packages/android's .so files + Kotlin
# bindings in from their build output (no second cross-compile). Run their
# generators first, then copy: kept self-contained (not referenced via a
# relative path) because relative paths across packages don't resolve
# reliably once this package is consumed through node_modules autolinking
# (an included Gradle build / CocoaPods pod behind a symlink).
set -euo pipefail
cd "$(dirname "$0")"
# COPY_ONLY=1 skips the Rust cross-compiles (other packages' build.sh, which
# other work depends on) when packages/ios and packages/android are already
# current, and only re-copies their output.
if [ "${COPY_ONLY:-0}" != "1" ]; then
  [ "${ONLY:-both}" = "android" ] || ../ios/build.sh
  [ "${ONLY:-both}" = "ios" ] || ../android/build.sh
fi

# ONLY=android|ios copies just that platform's artifacts in -- the CI jobs build one
# platform each, and the other's output isn't there to copy (scripts/ci-rn.sh).
copy_android() { [ "${ONLY:-both}" != "ios" ]; }
copy_ios() { [ "${ONLY:-both}" != "android" ]; }

if copy_android; then
echo "==> copying Android artifacts in"
# Only the copied `uniffi/` bindings -- never the whole kotlin/ tree (see
# packages/android/build.sh for the incident that made this explicit).
rm -rf android/src/main/kotlin/uniffi android/src/main/jniLibs
mkdir -p android/src/main/kotlin
cp -R ../android/src/main/kotlin/uniffi android/src/main/kotlin/
cp -R ../android/src/main/jniLibs android/src/main/jniLibs
# The Compose SinuaView + the voice sources it takes (packages/android/view,
# dev.sinua.voice) -- only these two copied packages, never the kotlin/ tree.
rm -rf android/src/main/kotlin/dev/sinua/view android/src/main/kotlin/dev/sinua/voice
mkdir -p android/src/main/kotlin/com/sinua
cp -R ../android/view/src/main/kotlin/dev/sinua/view android/src/main/kotlin/dev/sinua/view
# SinuaViewLayout is the XML/Views wrapper; it reads R.styleable from :sinua-view's
# own resources, which this module doesn't have (different namespace). RN hosts the
# Compose SinuaView directly (FxHostView.kt), so the file is dropped from the copy.
rm -f android/src/main/kotlin/dev/sinua/view/SinuaViewLayout.kt
cp -R ../android/src/main/kotlin/dev/sinua/voice android/src/main/kotlin/dev/sinua/voice
# The vendor voice sources (src/voice.ts). Gemini + ElevenLabs (and the shared
# OkHttp socket factory) are always compiled in; LiveKit and OpenAI Realtime go to
# their own source sets, which build.gradle.kts adds only when the app opts in
# through `sinua.voiceVendors`.
rm -rf android/src/main/kotlin/dev/sinua/gemini android/src/main/kotlin/dev/sinua/elevenlabs android/src/main/kotlin/dev/sinua/websocket
cp -R ../android/gemini/src/main/kotlin/dev/sinua/gemini android/src/main/kotlin/dev/sinua/gemini
cp -R ../android/elevenlabs/src/main/kotlin/dev/sinua/elevenlabs android/src/main/kotlin/dev/sinua/elevenlabs
cp -R ../android/websocket/src/main/kotlin/dev/sinua/websocket android/src/main/kotlin/dev/sinua/websocket
rm -rf android/src/livekit/kotlin/dev/sinua/livekit android/src/openai/kotlin/dev/sinua/openai
mkdir -p android/src/livekit/kotlin/com/sinua android/src/openai/kotlin/com/sinua
cp -R ../android/livekit/src/main/kotlin/dev/sinua/livekit android/src/livekit/kotlin/dev/sinua/livekit
cp -R ../android/openai/src/main/kotlin/dev/sinua/openai android/src/openai/kotlin/dev/sinua/openai

fi

if copy_ios; then
echo "==> copying iOS artifacts in"
rm -rf ios/CoreEngine core_engineFFI.xcframework
mkdir -p ios/CoreEngine
cp ../ios/Sources/CoreEngine/core_engine.swift ios/CoreEngine/
cp -R ../ios/core_engineFFI.xcframework core_engineFFI.xcframework
# The SwiftUI SinuaView, SinuaVoice and the voice types they share (SinuaVoiceTypes),
# flattened into this pod's one Swift module: their `import CoreEngine` /
# `import SinuaVoice` / `(@_exported) import SinuaVoiceTypes` lines are dropped.
rm -rf ios/Sinua ios/SinuaVoice ios/SinuaVoiceTypes ios/Vendors
mkdir -p ios/Sinua ios/SinuaVoice ios/SinuaVoiceTypes
drop_types_import() { sed -e '/^import SinuaVoiceTypes$/d' -e '/^@_exported import SinuaVoiceTypes$/d'; }
for f in ../ios/Sources/Sinua/*.swift; do sed -e '/^import CoreEngine$/d' -e '/^import SinuaVoice$/d' -e 's/SinuaVoice\.//g' "$f" | drop_types_import > "ios/Sinua/$(basename "$f")"; done
for f in ../ios/Sources/SinuaVoice/*.swift; do sed -e '/^import CoreEngine$/d' "$f" | drop_types_import > "ios/SinuaVoice/$(basename "$f")"; done
cp ../ios/Sources/SinuaVoiceTypes/*.swift ios/SinuaVoiceTypes/
# The vendor voice sources (src/voice.ts), flattened the same way. Gemini and
# ElevenLabs are Foundation-only and ship in the default pod; LiveKit and OpenAI
# Realtime sit in their own folders, which the SinuaCore/LiveKit and
# SinuaCore/OpenAI subspecs add (each with its SDK and a SINUA_* flag).
mkdir -p ios/Vendors/GeminiLive ios/Vendors/ElevenLabs ios/Vendors/LiveKit ios/Vendors/OpenAI
# `public` NSObject subclasses land in the pod's generated ObjC header, which the
# umbrella then compiles without the vendor SDK's modules (RoomDelegate,
# LKRTCPeerConnectionDelegate, ...). Nothing outside this pod uses these types --
# the registry does (same module) -- so the copies are module-internal.
demote_public() { sed -e 's/^public final class /final class /' -e 's/^public class /class /' -e 's/^public enum /enum /' -e 's/^public struct /struct /'; }
# Flattened into this pod's single Swift module: the module imports go, and so do
# `SinuaVoice.`-qualified type names (the pod has a class of that name -- the RN
# bridge module -- so the qualifier would resolve to it).
strip_imports() { sed -e '/^import CoreEngine$/d' -e '/^import SinuaVoice$/d' -e 's/SinuaVoice\.//g' "$1"; }
for f in ../ios/Sources/SinuaGeminiLive/*.swift; do strip_imports "$f" | demote_public > "ios/Vendors/GeminiLive/$(basename "$f")"; done
for f in ../ios/Sources/SinuaElevenLabs/*.swift; do strip_imports "$f" | demote_public > "ios/Vendors/ElevenLabs/$(basename "$f")"; done
# LiveKit's Swift module is `LiveKit` under SwiftPM but `LiveKitClient` under
# CocoaPods (its podspec sets no module_name), and this package is consumed as a pod.
for f in ../ios-livekit/Sources/SinuaLiveKit/*.swift; do
  strip_imports "$f" | sed -e 's/^import LiveKit$/import LiveKitClient/' | demote_public > "ios/Vendors/LiveKit/$(basename "$f")"
done
for f in ../ios-openai/Sources/SinuaOpenAI/*.swift; do strip_imports "$f" | demote_public > "ios/Vendors/OpenAI/$(basename "$f")"; done

fi

npm run build
echo "==> done. cd example && (cd ios && bundle exec pod install) or run-android to build the example app."
