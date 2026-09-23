plugins {
    id("com.android.library") version "8.7.3"
    id("org.jetbrains.kotlin.android") version "2.0.21"
}

android {
    // Published variant + the sources/javadoc jars Maven Central requires (docs/publishing.md).
    publishing {
        singleVariant("release") {
            withSourcesJar()
            withJavadocJar()
        }
    }

    namespace = "dev.sinua.core"
    // Matches the platform the .so files were built for (packages/android/build.sh:
    // cargo ndk --platform 24). Below a typical app's minSdk (26), so it never
    // constrains that app's own floor.
    compileSdk = 36

    defaultConfig {
        minSdk = 24
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    sourceSets {
        getByName("main") {
            kotlin.srcDirs("src/main/kotlin")
            jniLibs.srcDirs("src/main/jniLibs")
        }
        // The frozen vector sets ship inside the *test* APK, read straight
        // from the repo's spec/ -- no copy that could drift from the file
        // crates/core_engine/tests/sinua_golden.rs checks. Read with the
        // instrumentation context's assets (SinuaGoldenTests.kt).
        getByName("androidTest") {
            assets.srcDirs("../../spec")
        }
    }

    // The instrumented-test APK otherwise targets minSdk (24), and the shared
    // emulator shows an "app built for an older version of Android" dialog.
    testOptions {
        targetSdk = 36
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
    // UniFFI's Kotlin bindings call into the native library through JNA.
    implementation("net.java.dev.jna:jna:5.14.0@aar")

    testImplementation("junit:junit:4.13.2")
    // JVM unit tests (dev.sinua.voice parity vs spec/voice-golden.json) need a real
    // org.json -- android.jar's is a stub that throws off-device.
    testImplementation("org.json:json:20240303")
    androidTestImplementation("androidx.test.ext:junit:1.2.1")
    androidTestImplementation("androidx.test:runner:1.6.2")
}

// Maven metadata for a release (docs/publishing.md); nothing is published by a build.
extra["sinua.artifactId"] = "sinua-core"
extra["sinua.description"] = "The sinua geometry engine and voice types for Android (UniFFI bindings + .so)"
apply(from = rootProject.file("gradle/publish.gradle.kts"))
