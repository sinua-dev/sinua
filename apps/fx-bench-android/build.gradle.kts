// sinua device bench, Android (docs/bench.md). A subproject of
// packages/android's Gradle root (see the
// `include(":fx-bench-android")` block in packages/android/settings.gradle.kts).
// No AGP/Kotlin plugin versions here: they're on the root build's classpath.
plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose") version "2.0.21"
}

android {
    namespace = "dev.sinua.fxbench"
    compileSdk = 36

    defaultConfig {
        applicationId = "dev.sinua.fxbench"
        minSdk = 24
        targetSdk = 36
        versionCode = 1
        versionName = "1.0"
    }

    buildFeatures {
        compose = true
    }

    // Measure release code on a phone without a signing setup: release is
    // signed with the local debug key (the usual benchmark-build pattern;
    // this app is never published). Debug builds run Compose unoptimized.
    buildTypes {
        getByName("release") {
            signingConfig = signingConfigs.getByName("debug")
            isMinifyEnabled = false
        }
    }

    sourceSets {
        getByName("main") {
            kotlin.srcDirs("src/main/kotlin")
            // spec/bench/cases.json, shared with the Web and iOS benches.
            assets.srcDirs("../../spec/bench")
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
    implementation(project(":"))
    implementation(project(":sinua-view"))

    val composeBom = platform("androidx.compose:compose-bom:2024.10.01")
    implementation(composeBom)
    implementation("androidx.compose.ui:ui")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.activity:activity-compose:1.9.3")
    // JankStats (FrameMetrics-based, API 24+): the platform's own jank count per case.
    implementation("androidx.metrics:metrics-performance:1.0.0")
}
