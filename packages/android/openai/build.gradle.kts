// :sinua-openai -- the native OpenAI Realtime `VoiceSource` over WebRTC
// (docs/audio-pipeline.md, *Native OpenAI Realtime*). Its own module: it carries a
// WebRTC binary that SinuaView and the other vendors must never pull. WebRTC is the
// LiveKit-maintained prefixed build (`livekit.org.webrtc`) at the version
// livekit-android 2.28.2 uses, so an app with both resolves to one binary.
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

    namespace = "dev.sinua.openai"
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
    // dev.sinua.voice: VoiceSource, OpenAIRealtimeSession / Signaling, RealtimeReconnect, PcmTap.
    api(project(":"))
    // OkHttp 4.12.0 for the two signaling POSTs (shared with the other vendors).
    implementation(project(":sinua-websocket"))
    api("io.github.webrtc-sdk:android-prefixed:144.7559.14")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.6.0")

    testImplementation("junit:junit:4.13.2")
    testImplementation("com.squareup.okhttp3:mockwebserver:4.12.0")
    testImplementation("org.json:json:20240303")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.6.0")
}

// Maven metadata for a release (docs/publishing.md); nothing is published by a build.
extra["sinua.artifactId"] = "sinua-openai"
extra["sinua.description"] = "OpenAI Realtime VoiceSource over WebRTC"
apply(from = rootProject.file("gradle/publish.gradle.kts"))
