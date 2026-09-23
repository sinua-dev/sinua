# Sinua for Android

The Gradle build: the Rust geometry engine, a Jetpack Compose view, and the
voice sources that drive it. minSdk 24, compileSdk 36.

```kotlin
SinuaView(pattern = "breathing", size = 64)
```

## Modules

| Module | Artifact | What it is |
|---|---|---|
| root | `dev.sinua:sinua-core` | The UniFFI bindings over the Rust engine, plus `dev.sinua.voice`: microphone, test tone, `VoiceOverrides`, and the SDK-free half of every vendor adapter. |
| `:sinua-view` | | The Compose `SinuaView` and `SinuaViewLayout` (the XML/Views wrapper), and the paint contract. |
| `:sinua-websocket` | | The shared OkHttp socket factory the WebSocket vendors use. |
| `:sinua-gemini` | | Gemini Live over its WebSocket. |
| `:sinua-elevenlabs` | | ElevenLabs Agents over its WebSocket. |
| `:sinua-livekit` | | A LiveKit room's agent audio. Pulls `livekit-android`. |
| `:sinua-openai` | | OpenAI Realtime over WebRTC. Pulls LiveKit's prefixed WebRTC build, so an app using both vendors links one WebRTC binary rather than two. |

The vendor modules are separate so an app carries only the SDKs it uses. The
root module and `:sinua-view` depend on none of them.

`:fx-bench-android` and `:site-snippets` are also registered
here — they are apps and samples in this repository, not published artifacts.

## Adding it

Until the artifacts are published, include the build and depend on the modules
by project. Once published:

```kotlin
implementation("dev.sinua:sinua-core:0.1.0-beta.1")
implementation("dev.sinua:sinua-view:0.1.0-beta.1")
implementation("dev.sinua:sinua-gemini:0.1.0-beta.1")   // only the vendors you use
```

The POM's URL, SCM and developer fields default to the public repository
(`sinua-dev/sinua`) and "The Sinua Authors", each overridable by a `sinua.*`
Gradle property — see `gradle/publish.gradle.kts` and
[`docs/publishing.md`](../../docs/publishing.md).

`:sinua-livekit` needs JitPack for one transitive dependency (`audioswitch`);
`settings.gradle.kts` adds it filtered to that group only.

## Building and testing

```sh
./build.sh                                   # cargo-ndk + uniffi -> jniLibs + bindings
ANDROID_HOME=~/Library/Android/sdk ./gradlew :testDebugUnitTest :sinua-view:testDebugUnitTest
ANDROID_HOME=~/Library/Android/sdk ./gradlew :connectedDebugAndroidTest   # needs an emulator
```

`build.sh` deletes and regenerates exactly three paths — `bindings/kotlin`,
`src/main/kotlin/uniffi` and `src/main/jniLibs` — and nothing else. It once
removed the whole `src/main/kotlin` tree, taking hand-written sources with it,
which is why the scope is now narrow and spelled out in the script.

The geometry is checked against the same golden vectors as every other platform
(`spec/orbs-golden.json`, `spec/sinua-golden.json`), and the voice analysis
against byte-level fixtures captured from Chrome (`spec/voice-golden.json`).

## What is not verified

Everything here runs against fakes: fake audio devices, a test-driven clock, and
in-process fake servers on localhost. **No vendor session has ever run against
the real service, and no physical device has ever run this code** — one headless
emulator only, always started with `-no-audio`. The tests never open a
microphone or play sound.

## Licence

Apache-2.0. See [`LICENSE`](LICENSE) and [`NOTICE`](../../NOTICE).
