package dev.sinua.reactnative

import com.facebook.react.ReactPackage
import com.facebook.react.bridge.NativeModule
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.uimanager.ViewManager

class SinuaCorePackage : ReactPackage {
    override fun createNativeModules(reactContext: ReactApplicationContext): List<NativeModule> =
        // SinuaVoice: native voice sources created from JS (src/voice.ts).
        listOf(SinuaCoreModule(reactContext), SinuaVoiceModule(reactContext))

    // `<SinuaView>`: the Fabric component hosting the Compose SinuaView (SinuaViewManager).
    override fun createViewManagers(reactContext: ReactApplicationContext): List<ViewManager<*, *>> =
        listOf(SinuaViewManager())
}
