#!/usr/bin/env bash
# Regenerates the Swift bindings + core_engineFFI.xcframework (both
# gitignored build output) from crates/core_engine. Run this after any
# change to the Rust engine, before `swift build` / `xcodebuild`.
set -euo pipefail
cd "$(dirname "$0")"
ROOT="$(cd ../.. && pwd)"
BINDINGS="$ROOT/bindings/swift"

rustup target add aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios >/dev/null

echo "==> building host dylib (for bindgen introspection)"
(cd "$ROOT" && cargo build -p core_engine -p uniffi-bindgen)

echo "==> generating Swift bindings"
rm -rf "$BINDINGS"
mkdir -p "$BINDINGS/Headers"
(cd "$ROOT" && cargo run -p uniffi-bindgen -- generate \
  --library target/debug/libcore_engine.dylib \
  --language swift --out-dir bindings/swift)
cp "$BINDINGS/core_engineFFI.h" "$BINDINGS/Headers/"
cp "$BINDINGS/core_engineFFI.modulemap" "$BINDINGS/Headers/module.modulemap"
mkdir -p Sources/CoreEngine
cp "$BINDINGS/core_engine.swift" Sources/CoreEngine/

echo "==> building iOS device + simulator static libs"
for target in aarch64-apple-ios aarch64-apple-ios-sim x86_64-apple-ios; do
  (cd "$ROOT" && cargo build -p core_engine --release --target "$target")
done

echo "==> lipo-ing simulator slices into a universal lib"
mkdir -p "$ROOT/target/ios-sim-universal"
lipo -create \
  "$ROOT/target/aarch64-apple-ios-sim/release/libcore_engine.a" \
  "$ROOT/target/x86_64-apple-ios/release/libcore_engine.a" \
  -output "$ROOT/target/ios-sim-universal/libcore_engine.a"

# Debug info (DWARF) is ~84% of each static lib (device 23 MB -> 3.6 MB). An app never
# ships it (Xcode strips the final binary), but the release zip every consumer downloads
# would. `strip -S` drops only debug symbols; the global symbols the linker needs stay.
echo "==> stripping debug info from the static libs"
mkdir -p "$ROOT/target/ios-device"
cp "$ROOT/target/aarch64-apple-ios/release/libcore_engine.a" "$ROOT/target/ios-device/libcore_engine.a"
xcrun strip -S "$ROOT/target/ios-device/libcore_engine.a" "$ROOT/target/ios-sim-universal/libcore_engine.a"

echo "==> creating core_engineFFI.xcframework"
rm -rf core_engineFFI.xcframework
xcodebuild -create-xcframework \
  -library "$ROOT/target/ios-device/libcore_engine.a" -headers "$BINDINGS/Headers" \
  -library "$ROOT/target/ios-sim-universal/libcore_engine.a" -headers "$BINDINGS/Headers" \
  -output core_engineFFI.xcframework

echo "==> done. Try: swift build --triple arm64-apple-ios-simulator -sdk \$(xcrun --sdk iphonesimulator --show-sdk-path)"
