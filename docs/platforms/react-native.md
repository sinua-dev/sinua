# React Native

> **No renderer here.** This package proves `frame(state, size, t)` returns correct data through the bridge (see the example app's smoke test) — it does not draw anything. The original plan called for `@shopify/react-native-skia`; that hasn't been integrated. A consumer gets an `OrbFrame` (dots/lines) via `await frame(...)` and is on its own to paint it today. See [`../roadmap.md`](../roadmap.md).

## Why not wasm, why not a JS reimplementation

Two tempting shortcuts were considered and rejected:

- **Reuse the wasm build**, the way `packages/core` does for plain Web. Rejected: RN's JS engine (Hermes) doesn't run WebAssembly maturely, and more fundamentally, RN doesn't need to fall back to wasm at all — an RN app is a real native app with a real iOS/Android runtime underneath, so it can link against actual native binaries the way Swift/Kotlin do.
- **Reimplement the engine in pure JS/TypeScript**, the way upstream `thinking-orbs` actually does it for *its own* React Native port (`ports/react-native/thinking-orbs-native` imports the same TS engine module and runs it in a Reanimated worklet — no native code at all). This works for upstream because their canonical source *is* TypeScript. It does not work here: this project deliberately moved the canonical implementation to Rust specifically to eliminate multi-platform drift (see [`../architecture.md`](../architecture.md)) — writing a second, hand-maintained JS copy just for RN would reintroduce exactly that risk for exactly one platform.

The chosen design: `packages/react-native` is a thin bridge module that calls the **same native `core_engine` binaries** `packages/ios` and `packages/android` already build and test — no separate Rust compilation, no separate math.

## Classic bridge, not JSI/TurboModule — a deliberate scope decision

RN's "new architecture" (Fabric + TurboModules + JSI) gives true synchronous JS↔native calls with no serialization overhead, which is the technically ideal fit for a per-frame animation call. It was considered and explicitly deferred in favor of the classic Promise-based bridge (`@ReactMethod`/`RCT_EXTERN_METHOD`), for a concrete reason: a TurboModule needs codegen config, C++ glue on both platforms, and a real example app to validate against — substantially more new surface area, for a project with (at the time) no existing RN app to build it against. The classic bridge:

- Reuses the exact same JSON-over-the-wire tradeoff already accepted for the Web/wasm path (see [`../architecture.md`](../architecture.md#the-uniffi--wasm-bindgen-split)) — a consistent engineering call, not a special case.
- Can be swapped for a TurboModule later **without touching the native `core_engine` layer at all** — it's purely an interface-layer change.

If per-frame call overhead ever becomes a measured problem (not a guessed one), that's the trigger to revisit this, not before.

## Package layout

```
packages/react-native/
├── SinuaCore.podspec              # iOS: vendors core_engineFFI.xcframework + core_engine.swift
├── ios/
│   ├── SinuaCore.swift             # bridge module (own code, tracked)
│   ├── SinuaCore.m                 # RCT_EXTERN_METHOD registration (own code, tracked)
│   └── CoreEngine/core_engine.swift  # gitignored, copied in from packages/ios by build.sh
├── android/
│   ├── build.gradle.kts
│   └── src/main/
│       ├── java/.../SinuaCoreModule.kt   # bridge module (own code, tracked)
│       ├── java/.../SinuaCorePackage.kt  # ReactPackage registration (own code, tracked)
│       ├── kotlin/uniffi/                   # gitignored, copied in from packages/android
│       └── jniLibs/                          # gitignored, copied in from packages/android
├── src/index.ts                      # TS wrapper: frame, frameWithOverrides, resolveFxSpec/frameFromFxSpec (optional { state, inputs } ctx, 1.1)/fxColorToHsl (FX Spec, ../fx-spec.md)
└── example/                          # RN CLI-scaffolded app, used to validate the bridge end-to-end
```

The `ios/` and `android/kotlin,jniLibs/` artifacts are **copied**, not cross-compiled a second time or referenced via a relative path across packages — `packages/react-native/build.sh` runs `packages/ios/build.sh` and `packages/android/build.sh` first, then copies their output in. A relative-path reference was tried first and rejected: it doesn't resolve reliably once this package is consumed through `node_modules` autolinking, which puts it behind a symlink inside an "included Gradle build" / CocoaPods local-path context that doesn't preserve relative paths the way a normal same-repo reference would.

## Bridge method naming

Both `ios/SinuaCore.swift` and `android/.../SinuaCoreModule.kt` name their exposed methods `resolveFrame` / `resolveFrameWithOverrides`, **not** `frame` / `frameWithOverrides` — because the vendored `core_engine.swift` / `core_engine.kt` files already define free functions with those exact names at module scope, and a same-named instance method would shadow them (an unqualified call to `frame(...)` from inside a method literally named `frame` self-recurses instead of calling the free function). `src/index.ts` renames back to `frame`/`frameWithOverrides` for the public TypeScript API, so this is invisible to a consumer — it's purely an internal naming collision avoidance.

## The example app

`example/` is a real RN CLI-scaffolded app (`npx @react-native-community/cli init`), not a toy. Its `App.tsx` is a live smoke test: calls `frame()` and `frameWithOverrides()` on mount, compares against a `spec/orbs-golden.json` value, and renders PASS/FAIL on screen — checked by installing on both an iOS Simulator and an Android emulator and reading the actual rendered text (see [`../lessons-learned.md`](../lessons-learned.md) for two real bugs this caught that a compile-only check would have missed).

Two setup quirks specific to this app, both because `@sinua/react-native` is a `file:`-linked sibling package rather than a real `node_modules` dependency:

- **Metro needs `watchFolders`** pointed at `packages/react-native` (`example/metro.config.js`) — Metro doesn't watch or resolve outside its project root by default, even through a symlink, so without this the app fails to resolve the `@sinua/react-native` import at all.
- **The example's root `android/build.gradle` SDK versions were pinned down** from the RN template's defaults (compileSdk 37, NDK 27.1.x) to versions already installed on this machine (compileSdk 36, NDK 27.0.12077973) — purely to avoid a multi-GB fresh SDK download for what is a smoke test, not a compatibility requirement.

## Running the smoke test

```bash
packages/react-native/build.sh   # rebuilds ios/ + android/ first, copies artifacts in, builds the TS wrapper

# Android
$ANDROID_HOME/emulator/emulator -avd <name> -no-window &
cd example && npx react-native run-android --no-packager
adb reverse tcp:8081 tcp:8081 && npx react-native start &   # Metro, separately
adb logcat -d | grep "Sinua smoke"

# iOS
cd example/ios && pod install
xcodebuild -workspace ios/SinuaExample.xcworkspace -scheme SinuaExample \
  -destination 'platform=iOS Simulator,name=<device>' -derivedDataPath ios/build build
xcrun simctl install <udid> ios/build/.../SinuaExample.app
xcrun simctl terminate <udid> <bundle-id>   # see lessons-learned.md -- launch alone won't pick up a rebuild
xcrun simctl launch <udid> <bundle-id>
xcrun simctl io <udid> screenshot out.png   # read the result visually, no on-device log capture proved reliable
```
