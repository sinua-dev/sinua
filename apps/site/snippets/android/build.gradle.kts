// :site-snippets -- compiles the docs' Kotlin + Android XML samples
// (apps/site/snippets/**/*.kt, platforms/android-res) against the local modules,
// so a sample that drifts from the API fails CI (scripts/ci-native-android.sh build).
// A library nobody depends on; registered in packages/android/settings.gradle.kts.
plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose") version "2.0.21"
}

android {
    namespace = "snippets"
    compileSdk = 36
    defaultConfig { minSdk = 24 }
    buildFeatures { compose = true }
    compileOptions {
        sourceCompatibility = JavaVersion.VERSION_17
        targetCompatibility = JavaVersion.VERSION_17
    }
    kotlinOptions { jvmTarget = "17" }
    sourceSets {
        getByName("main") {
            // Only *.kt compiles; the folders' Swift / TS samples are ignored.
            kotlin.srcDirs(listOf("first-visual", "platforms", "values", "states", "bindings", "voice", "perf").map { "../$it" })
            res.srcDirs("../platforms/android-res")
        }
    }
}

dependencies {
    implementation(project(":sinua-view"))
    implementation(project(":sinua-livekit"))
    implementation(project(":sinua-gemini"))
    implementation(project(":sinua-elevenlabs"))
    implementation(project(":sinua-openai"))

    val composeBom = platform("androidx.compose:compose-bom:2024.10.01")
    implementation(composeBom)
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.activity:activity-ktx:1.9.3")
}
