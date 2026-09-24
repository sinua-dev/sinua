plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
    // The Compose SinuaView (copied in by build.sh). Consumers' buildscript needs
    // org.jetbrains.kotlin:compose-compiler-gradle-plugin at their Kotlin version.
    id("org.jetbrains.kotlin.plugin.compose")
    // Fabric Codegen for <SinuaView> (src/specs/SinuaViewNativeComponent.ts).
    id("com.facebook.react")
}

react {
    jsRootDir = file("../src/specs")
    libraryName = "SinuaViewSpec"
    codegenJavaPackageName = "dev.sinua.reactnative"
}

// Voice vendors are opt-in: an app adds them in its gradle.properties, e.g.
//   sinua.voiceVendors=gemini,elevenlabs,livekit,openai
// Gemini and ElevenLabs need only OkHttp and are on by default; LiveKit pulls
// livekit-android and OpenAI Realtime pulls the prefixed WebRTC binary, so an app
// that doesn't use them never downloads them (docs/fx-view.md, *React Native*).
val voiceVendors: Set<String> = ((project.findProperty("sinua.voiceVendors") as String?) ?: "gemini,elevenlabs")
    .split(",").map { it.trim().lowercase() }.filter { it.isNotEmpty() }.toSet()

android {
    namespace = "dev.sinua.reactnative"
    compileSdk = 36

    defaultConfig {
        // JNA + UniFFI keep rules for apps built with R8 (see the file).
        consumerProguardFiles("consumer-rules.pro")
        minSdk = 24
    }

    sourceSets {
        getByName("main") {
            // build.sh copies the UniFFI-generated Kotlin bindings + .so
            // files in from packages/android's build output (no second
            // cross-compile) -- kept self-contained rather than referenced
            // via a relative path, which doesn't resolve reliably once this
            // package is consumed through node_modules autolinking (an
            // included Gradle build behind a symlink).
            kotlin.srcDirs(
                listOf("src/main/java", "src/main/kotlin") +
                    voiceVendors.filter { it == "livekit" || it == "openai" }.map { "src/$it/kotlin" },
            )
            jniLibs.srcDirs("src/main/jniLibs")
        }
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
    implementation("com.facebook.react:react-android")
    implementation("net.java.dev.jna:jna:5.14.0@aar")

    // The Compose SinuaView -- the same BOM/libraries as packages/android/view.
    val composeBom = platform("androidx.compose:compose-bom:2024.10.01")
    implementation(composeBom)
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-graphics")
    implementation("androidx.compose.foundation:foundation")
    // The copied SinuaView frame loop uses withInfiniteAnimationFrameNanos (as packages/android/view).
    implementation("androidx.compose.animation:animation-core")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.7")

    // The voice sources: dev.sinua.voice + the vendor glue copied in by build.sh.
    // Gemini / ElevenLabs speak WebSocket through OkHttp (:sinua-websocket's factory).
    implementation("com.squareup.okhttp3:okhttp:4.12.0")
    if ("livekit" in voiceVendors) implementation("io.livekit:livekit-android:2.28.2")
    // OpenAI Realtime's WebRTC: the same prefixed build LiveKit uses, so the two share it.
    if ("openai" in voiceVendors) implementation("io.github.webrtc-sdk:android-prefixed:144.7559.14")
    if ("livekit" in voiceVendors || "openai" in voiceVendors) {
        implementation("org.jetbrains.kotlinx:kotlinx-coroutines-android:1.6.0")
    }
}
