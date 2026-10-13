// A release build with R8 on, exactly as an app ships to Play, that draws one SinuaView and
// starts WebRTC the way sinua-openai's connect() does. It carries no ProGuard rules of its own:
// everything must come from the modules' consumer rules (sinua-core's for the engine,
// sinua-openai's for WebRTC). gemini and elevenlabs ride along so their rules are proven too;
// sinua-livekit stays out on purpose (livekit-android's own rules would mask a gap here). The CI runs it on
// the emulator through scripts/android-minify-smoke.sh. Added after 0.1.0-beta.4, whose
// minified apps threw UnsatisfiedLinkError in JNA on the first engine call.
plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose") version "2.0.21"
}

android {
    namespace = "dev.sinua.minifysmoke"
    compileSdk = 36

    defaultConfig {
        applicationId = "dev.sinua.minifysmoke"
        minSdk = 24
        targetSdk = 36
        versionCode = 1
        versionName = "1.0"
    }

    buildFeatures {
        compose = true
    }

    buildTypes {
        getByName("release") {
            isMinifyEnabled = true
            proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"))
            signingConfig = signingConfigs.getByName("debug")
        }
    }

    sourceSets {
        getByName("main") {
            kotlin.srcDirs("src/main/kotlin")
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
    implementation(project(":sinua-view"))
    implementation(project(":sinua-openai"))
    implementation(project(":sinua-gemini"))
    implementation(project(":sinua-elevenlabs"))

    val composeBom = platform("androidx.compose:compose-bom:2024.10.01")
    implementation(composeBom)
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.activity:activity-compose:1.9.3")
}
