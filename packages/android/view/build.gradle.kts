// :sinua-view -- the drop-in Compose renderer (SinuaView + the shared paint
// contract, docs/fx-view.md). A separate module so the core library
// (packages/android root) stays Compose-free for apps that only want the
// geometry. Same plugin/BOM versions as the other Compose modules (plugins are
// already on this Gradle build's classpath, see its build.gradle.kts).
plugins {
    id("com.android.library")
    id("org.jetbrains.kotlin.android")
    id("org.jetbrains.kotlin.plugin.compose") version "2.0.21"
}

android {
    // Published variant + the sources/javadoc jars Maven Central requires (docs/publishing.md).
    publishing {
        singleVariant("release") {
            withSourcesJar()
            withJavadocJar()
        }
    }

    namespace = "dev.sinua.view"
    compileSdk = 36

    defaultConfig {
        minSdk = 24
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
    }

    buildFeatures {
        compose = true
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

    sourceSets {
        // Example specs for the instrumented render test, read from the repo's spec/,
        // plus spec/ itself for parameters.json -- the pattern list SinuaViewTest
        // renders, read rather than copied so it cannot drift from the catalog.
        // (Examples then also appear under examples/; the render test lists the
        // asset root only, so it sees each example once.)
        getByName("androidTest") {
            assets.srcDirs("../../../spec/examples", "../../../spec")
        }
    }
}

dependencies {
    // The core library: uniffi.core_engine (engine + FX Spec) and dev.sinua.voice.
    api(project(":"))

    val composeBom = platform("androidx.compose:compose-bom:2024.10.01")
    // `api`, not `implementation`: SinuaView / the typed components take a Compose
    // `Modifier` in their public signatures, so compose-ui must be on the consumer's
    // compile classpath (the POM's `compile` scope). Found by the release dry run,
    // 2026-09-21: an app without its own compose-ui couldn't compile a SinuaOrb call.
    api(composeBom)
    api("androidx.compose.ui:ui")
    implementation("androidx.compose.ui:ui-graphics")
    implementation("androidx.compose.foundation:foundation")
    // withInfiniteAnimationFrameNanos: the frame loop is an infinite animation (idle-friendly in tests).
    implementation("androidx.compose.animation:animation-core")
    implementation("androidx.lifecycle:lifecycle-runtime-compose:2.8.7")

    testImplementation("junit:junit:4.13.2")
    // android.jar's org.json is a stub off-device (the typed components' spec check).
    testImplementation("org.json:json:20240303")
    androidTestImplementation(composeBom)
    androidTestImplementation("androidx.compose.ui:ui-test-junit4")
    androidTestImplementation("androidx.test.ext:junit:1.2.1")
    androidTestImplementation("androidx.test:runner:1.6.2")
    debugImplementation("androidx.compose.ui:ui-test-manifest")
}

// Maven metadata for a release (docs/publishing.md); nothing is published by a build.
extra["sinua.artifactId"] = "sinua-view"
extra["sinua.description"] = "The drop-in Compose renderer: SinuaView, SinuaViewLayout and the typed components"
apply(from = rootProject.file("gradle/publish.gradle.kts"))
