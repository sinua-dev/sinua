#!/usr/bin/env bash
# Regenerates the Kotlin bindings + jniLibs .so files (both gitignored build
# output) from crates/core_engine. Run this after any change to the Rust
# engine, before `./gradlew build`.
set -euo pipefail
cd "$(dirname "$0")"
ROOT="$(cd ../.. && pwd)"
: "${ANDROID_NDK_HOME:=/opt/homebrew/share/android-commandlinetools/ndk/27.0.12077973}"
export ANDROID_NDK_HOME
# The Studio / dev exports (cargo feature `dev`, design note 34: cost,
# the parameter catalog, override checks) are in development builds -- the tests and the
# Studios use them -- and left out of what is published: the release workflow sets
# SINUA_NATIVE_RELEASE=1.
if [ "${SINUA_NATIVE_RELEASE:-0}" = 1 ]; then DEV=(); else DEV=(--features core_engine/dev); fi

rustup target add aarch64-linux-android armv7-linux-androideabi x86_64-linux-android i686-linux-android >/dev/null

echo "==> building host library (for bindgen introspection)"
(cd "$ROOT" && cargo build -p core_engine -p uniffi-bindgen ${DEV[@]+"${DEV[@]}"})
# The host cdylib is .dylib on macOS, .so on Linux (the CI android job runs on ubuntu).
case "$(uname -s)" in
  Darwin) HOST_LIB=libcore_engine.dylib ;;
  *)      HOST_LIB=libcore_engine.so ;;
esac

echo "==> generating Kotlin bindings"
# Only the generated `uniffi/` package is ours to replace. src/main/kotlin
# also holds hand-written library code (dev/sinua/voice/, voice-adapters'
# native audio pipeline) -- wiping the whole directory deleted it once
# (2026-09-18, restored from git).
rm -rf "$ROOT/bindings/kotlin" src/main/kotlin/uniffi
(cd "$ROOT" && cargo run -p uniffi-bindgen -- generate \
  --library "target/debug/$HOST_LIB" \
  --language kotlin --out-dir bindings/kotlin)
mkdir -p src/main/kotlin
cp -R "$ROOT/bindings/kotlin/uniffi" src/main/kotlin/

echo "==> cross-compiling for arm64-v8a, armeabi-v7a, x86_64, x86"
rm -rf src/main/jniLibs
# 16 KB pages (inbox J1): the 64-bit libraries are already 16384-aligned; the 32-bit
# ones get the same alignment (Play doesn't require it there, it costs nothing).
CARGO_TARGET_ARMV7_LINUX_ANDROIDEABI_RUSTFLAGS="-C link-arg=-Wl,-z,max-page-size=16384" \
CARGO_TARGET_I686_LINUX_ANDROID_RUSTFLAGS="-C link-arg=-Wl,-z,max-page-size=16384" \
cargo ndk \
  -t arm64-v8a -t armeabi-v7a -t x86_64 -t x86 \
  -o src/main/jniLibs \
  --platform 24 \
  build --release --manifest-path "$ROOT/crates/core_engine/Cargo.toml" ${DEV[@]+"${DEV[@]}"}

echo "==> done. Try: ./gradlew testDebugUnitTest"
