// :sinua-websocket -- the one OkHttp `LiveSocketFactory` shared by the native
// vendor VoiceSources (:sinua-gemini, :sinua-elevenlabs), so an app using
// both gets a single OkHttp. The interface itself is dependency-free in
// dev.sinua.voice.
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

    namespace = "dev.sinua.websocket"
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
    api(project(":"))
    // OkHttp 4.12.0, not the latest 5.5.0: 5.x needs kotlin-stdlib 2.2 and this
    // build compiles with Kotlin 2.0.21. 4.12.0 is also what livekit-android
    // 2.28.2 and elevenlabs-android 0.12.2 use.
    api("com.squareup.okhttp3:okhttp:4.12.0")
}

// Maven metadata for a release (docs/publishing.md); nothing is published by a build.
extra["sinua.artifactId"] = "sinua-websocket"
extra["sinua.description"] = "The shared OkHttp LiveSocketFactory for the vendor voice sources"
apply(from = rootProject.file("gradle/publish.gradle.kts"))
