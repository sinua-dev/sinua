pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
        // :sinua-livekit only: livekit-android needs JitPack for audioswitch
        // (its README). Filtered to that group so nothing else resolves there.
        maven("https://jitpack.io") {
            content { includeGroup("com.github.davidliu") }
        }
    }
}

rootProject.name = "sinua-android"

// The device bench (docs/bench.md, families): apps/fx-bench-android, wired
// in as a subproject.
include(":fx-bench-android")
project(":fx-bench-android").projectDir = file("$rootDir/../../apps/fx-bench-android")

// The drop-in Compose renderer (SinuaView + the shared paint contract). Its own
// module so this library stays Compose-free -- see view/build.gradle.kts.
include(":sinua-view")
project(":sinua-view").projectDir = file("$rootDir/view")

// The native LiveKit VoiceSource (docs/audio-pipeline.md, *Native LiveKit*). Its
// own module so the core library and :sinua-view never pull the LiveKit SDK.
include(":sinua-livekit")
project(":sinua-livekit").projectDir = file("$rootDir/livekit")

// The shared OkHttp LiveSocketFactory for the native vendor VoiceSources
// (Android has no platform WebSocket); one OkHttp for Gemini + ElevenLabs.
include(":sinua-websocket")
project(":sinua-websocket").projectDir = file("$rootDir/websocket")

// The native Gemini Live VoiceSource (docs/audio-pipeline.md, *Native Gemini
// Live*). Its own module, on :sinua-websocket.
include(":sinua-gemini")
project(":sinua-gemini").projectDir = file("$rootDir/gemini")

// The native ElevenLabs Agents VoiceSource (docs/audio-pipeline.md, *Native
// ElevenLabs*), on :sinua-websocket.
include(":sinua-elevenlabs")
project(":sinua-elevenlabs").projectDir = file("$rootDir/elevenlabs")

// The native OpenAI Realtime VoiceSource over WebRTC (docs/audio-pipeline.md,
// *Native OpenAI Realtime*): its own module, it carries the WebRTC binary.
include(":sinua-openai")
project(":sinua-openai").projectDir = file("$rootDir/openai")

// The docs site's Kotlin/XML code samples, compiled so they can't drift from the
// API (apps/site/snippets/android/build.gradle.kts). Nothing depends on it.
include(":site-snippets")
project(":site-snippets").projectDir = file("$rootDir/../../apps/site/snippets/android")
