# Changelog

Every published package (`@sinua/core`, `@sinua/web`, `@sinua/voice` on npm;
`dev.sinua:sinua-*` on Maven Central; `sinua-swift`, `-livekit`, `-openai` for SwiftPM)
shares one version, from the root `VERSION` file. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/). A `-beta.N` version is a public prerelease: npm
tag `beta`, and it may still change incompatibly.

How to release: [`docs/publishing.md`](docs/publishing.md), *How to release*.

## Unreleased

## 0.1.0-beta.1

The first public beta: npm (`@sinua/core`, `@sinua/web`, `@sinua/voice`, `beta` tag) and
SwiftPM (`sinua-swift`, `-livekit`, `-openai`). The Android packages (`dev.sinua:sinua-*`
on Maven Central) follow with `0.1.0-beta.2`, once the `dev.sinua` namespace is verified.

### Added

- The runtime: one Rust engine (`core_engine`) drawn on every platform. On the web it is
  WebAssembly: `@sinua/core`, and `@sinua/web` with `<sinua-view>` and typed components.
  On iOS it is the SwiftUI view `SinuaView` in the `Sinua` product. On Android it is
  `sinua-view` for Compose.
- FX Spec 1.8: the file format the Studio exports and every runtime reads. It has 34
  patterns, and each supports four voice states (idle, listening, thinking, speaking).
  1.8 is the oldest version the runtime reads. A key added in a later minor is an error
  in a file that declares an older one, and is dropped rather than silently applied.
- `glowing`: `depthTone` (0..1, default 1). At 0 every dot has the same tone; the sphere
  keeps its depth through dot size and opacity. At 1 it draws exactly as before.
- Voice: `@sinua/voice`, `SinuaVoice` and `sinua-core` drive the four states from an
  audio source. The adapters for Gemini Live, ElevenLabs, LiveKit and OpenAI Realtime are
  beta. On iOS the voice types are a separate product, so an app that only draws doesn't
  link microphone code.
- Typed components generated from the parameter catalog on every platform, with the
  public API frozen by a snapshot.

### Deprecated

- `@sinua/core`'s three JSON-bridge frame functions, each naming its direct twin.
