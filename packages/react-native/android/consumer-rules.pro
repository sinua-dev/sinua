# Consumer R8/ProGuard rules for the React Native package (same engine layer as dev.sinua:sinua-core), applied to every app that
# depends on it (also through sinua-view and the voice modules).
#
# UniFFI's Kotlin bindings talk to the Rust engine through JNA. JNA's native side
# (libjnidispatch.so) looks classes, fields and methods up by name (for example
# `com.sun.jna.Pointer.peer`), and fills the generated `Structure` subclasses
# reflectively. If R8 renames or strips them, the first engine call throws
# UnsatisfiedLinkError ("Can't obtain peer field ID for class com.sun.jna.Pointer")
# in a minified release build. JNA's own AAR ships no rules, so they live here.
-keep class com.sun.jna.** { *; }
-keep class * implements com.sun.jna.** { *; }
-dontwarn java.awt.**

# The generated bindings: Structure fields and callback interfaces are read by name.
-keep class uniffi.core_engine.** { *; }

# WebRTC (sinua.voiceVendors with openai or livekit): the same rules as sinua-openai's
# (packages/android/openai/consumer-rules.pro). The prefixed WebRTC binary looks its Java side up
# by name through JNI; if R8 strips or renames it, connect() aborts natively in a minified build.
# Harmless when no WebRTC vendor is built in.
-keepclasseswithmembernames,includedescriptorclasses class livekit.org.webrtc.** {
    native <methods>;
}
-keepclasseswithmembers,includedescriptorclasses class * {
    @livekit.**.CalledByNative <methods>;
}
-keepclasseswithmembers,includedescriptorclasses class * {
    @livekit.**.CalledByNativeUnchecked <methods>;
}
