# Development setup

Everything below was installed once, on one machine, in the order given — later steps sometimes depend on earlier ones being in place (e.g. `cargo-ndk` needs `rustup` first). If you're setting up a fresh machine, follow this order rather than jumping to the platform you care about.

## Rust (all platforms need this)

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs -o rustup-init.sh
sh rustup-init.sh -y --default-toolchain stable --profile default
```

This repo pins its own toolchain via the workspace-root `rust-toolchain.toml` (`channel = "1.98.1"`, plus every target every platform needs: `wasm32-unknown-unknown`, `aarch64-apple-ios`, `aarch64-apple-ios-sim`, `x86_64-apple-ios`, `aarch64-linux-android`, `armv7-linux-androideabi`, `x86_64-linux-android`, `i686-linux-android`). rustup treats a pinned channel as a **separate toolchain installation** from whatever `rustup default` points at — `rustup target add <target>` without `--toolchain <pinned-version>` adds to the wrong one, and a build inside this repo will fail with a confusing "can't find crate for `std`" even though the target appears installed. If you hit that, check `rustup target list --installed --toolchain 1.98.1` specifically.

```bash
cargo install wasm-pack   # or the official installer script; needed for packages/core
cargo install cargo-ndk   # needed for Android only, see below
```

## Web only

Nothing beyond Rust + `wasm-pack` + Node/npm.

## iOS only

Xcode + command-line tools (`xcode-select --install`). No CocoaPods needed for `packages/ios` itself (it's a plain Swift Package); CocoaPods is only needed for `packages/react-native/example`'s iOS side, since RN's iOS integration is CocoaPods-based:

```bash
cd packages/react-native/example/ios && pod install
```

`packages/ios/build.sh` needs the iOS Rust targets, which are already listed in `rust-toolchain.toml` — running the script installs them (`rustup target add ... --toolchain 1.98.1`, the pinned one) if missing.


## Android only

1. Android SDK (`sdkmanager`, `platform-tools`, at least one `platforms;android-NN`). This repo's setup uses a Homebrew-installed SDK at `/opt/homebrew/share/android-commandlinetools`.
2. **NDK**, installed once via `sdkmanager --install "ndk;27.0.12077973"` — not bundled with the SDK by default. Lands at `<sdk-root>/ndk/27.0.12077973`. `packages/android/build.sh` and `packages/react-native/build.sh` both default `ANDROID_NDK_HOME` to this exact path; override the env var if yours differs.
3. `cargo-ndk` (installed above) — wraps `cargo build` with the NDK's per-ABI linker flags. Do not hand-write `.cargo/config.toml` linker entries per target; this is what `cargo-ndk` exists to avoid.
4. A JDK — this setup uses `/opt/homebrew/Cellar/openjdk@17/17.0.20.1/.../Home` (OpenJDK 17).
5. An emulator or device for instrumented tests: `$ANDROID_HOME/emulator/emulator -avd <name> -no-window -no-audio -no-boot-anim &`, then poll `adb shell getprop sys.boot_completed` until it prints `1` (don't assume boot is done just because the emulator process started).

## React Native

Everything above (needs both iOS and Android toolchains, since `packages/react-native` bridges both), plus:

```bash
npx @react-native-community/cli@latest init <name> --directory example
```

was used to scaffold `packages/react-native/example` — note it also runs `git init` inside the new directory by default; that nested repo was removed immediately (a monorepo doesn't want a repo-within-a-repo). If re-scaffolding, remove `example/.git` the same way before doing anything else in it.

## Verifying a fresh setup end-to-end

There's no single "run this one command" check across all four platforms — see [`testing.md`](testing.md) for why (each platform's test needs its own runtime: a browser/Node for wasm, an iOS Simulator, an Android emulator). The fastest per-platform smoke checks:

```bash
cd crates/core_engine && cargo test                          # Rust + golden vectors, no other toolchain needed
cd packages/core && npm run build && node -e "..."            # see platforms/web.md
cd packages/ios && ./build.sh && xcodebuild test -scheme CoreEngine-Package -only-testing:CoreEngineTests -destination '...'
cd packages/android && ./build.sh && ./gradlew connectedDebugAndroidTest   # needs a booted emulator
cd packages/react-native && ./build.sh                        # then run the example app, see platforms/react-native.md
```
