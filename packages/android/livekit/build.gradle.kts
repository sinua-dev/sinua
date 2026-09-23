// :sinua-livekit -- the native LiveKit `VoiceSource` (docs/audio-pipeline.md,
// *Native LiveKit*). A separate module so the core library and :sinua-view
// never pull the LiveKit SDK (and its WebRTC native libs) into apps that
// don't use it. The SDK-free logic (`LiveKitAgentTracker`) lives in
// dev.sinua.voice and is JVM-tested there; this module is only the
// Room/track glue.
plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
}

android {
    // Published variant + the sources/javadoc jars Maven Central requires (docs/publishing.md).
    publishing {
        singleVariant("release") {
            withSourcesJar()
            withJavadocJar()
        }
    }

    namespace = "dev.sinua.livekit"
    compileSdk = 36

    defaultConfig {
        minSdk = 24
    }

    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions {
        jvmTarget = "17"
    }
}

dependencies {
    // dev.sinua.voice: VoiceSource, LiveKitAgentTracker.
    api(project(":"))
    // client-sdk-android v2.28.2 (2026-09-07). `api`: callers pass their own Room.
    api("io.livekit:livekit-android:2.28.2")
    // The SDK's own floor (its libs.versions.toml); it keeps coroutines `implementation`-scoped.
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.6.0")
}

// Maven metadata for a release (docs/publishing.md); nothing is published by a build.
extra["sinua.artifactId"] = "sinua-livekit"
extra["sinua.description"] = "LiveKit VoiceSource"
apply(from = rootProject.file("gradle/publish.gradle.kts"))
