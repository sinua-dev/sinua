# Consumer R8/ProGuard rules for dev.sinua:sinua-openai, applied to every app that depends on it.
#
# The prefixed WebRTC binary (io.github.webrtc-sdk:android-prefixed) looks its Java side up by
# name through JNI: JNI_OnLoad finds livekit.org.jni_zero.JniInit, and native code calls back
# into @CalledByNative methods all over livekit.org.webrtc. If R8 strips or renames them, the
# first PeerConnectionFactory call aborts natively (SIGTRAP, no Java exception) in a minified
# release build. The WebRTC AAR ships no rules, so they live here: the same rules livekit-android
# ships for this binary. scripts/android-r8-jni-check.mjs and the minified smoke app hold them.

# JNI native method names and descriptors (Java -> native).
-keepclasseswithmembernames,includedescriptorclasses class livekit.org.webrtc.** {
    native <methods>;
}

# Java methods called from native code (native -> Java), and their classes.
-keepclasseswithmembers,includedescriptorclasses class * {
    @livekit.**.CalledByNative <methods>;
}
-keepclasseswithmembers,includedescriptorclasses class * {
    @livekit.**.CalledByNativeUnchecked <methods>;
}
