// :sinua-elevenlabs -- the native ElevenLabs Agents `VoiceSource`
// (docs/audio-pipeline.md, *Native ElevenLabs*). Raw WebSocket (not ElevenLabs'
// own Android SDK, which carries voice over LiveKit and keeps its Room
// private); its own module so the core library never forces a network stack.
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

    namespace = "dev.sinua.elevenlabs"
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
    // dev.sinua.voice: VoiceSource, ElevenLabsSession, PcmAudioGraph, AndroidPcmAudioDevice.
    api(project(":"))
    // The shared OkHttp socket (OkHttp 4.12.0). `api`: its ClosedException reaches
    // apps through onError, so it's part of this module's error surface.
    api(project(":sinua-websocket"))

    testImplementation("junit:junit:4.13.2")
    testImplementation("com.squareup.okhttp3:mockwebserver:4.12.0")
    // android.jar's org.json is a stub off-device.
    testImplementation("org.json:json:20240303")
}

// Maven metadata for a release (docs/publishing.md); nothing is published by a build.
extra["sinua.artifactId"] = "sinua-elevenlabs"
extra["sinua.description"] = "ElevenLabs Agents VoiceSource"
apply(from = rootProject.file("gradle/publish.gradle.kts"))
