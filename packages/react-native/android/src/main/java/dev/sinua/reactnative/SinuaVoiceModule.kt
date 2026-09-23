package dev.sinua.reactnative

import com.facebook.react.bridge.Arguments
import com.facebook.react.bridge.Promise
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod
import com.facebook.react.bridge.ReadableMap
import com.facebook.react.modules.core.DeviceEventManagerModule
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.launch
import kotlinx.coroutines.withContext

/**
 * The React Native bridge for native voice sources (src/voice.ts). It only forwards
 * to [VoiceRegistry] -- create / connect / disconnect / release and the
 * `getCredential` round trip -- and streams the registry's events to JS as one
 * `sinua-voice` event carrying the source's id.
 *
 * Credentials arrive in `create` (or per request) and go straight into the source;
 * this class keeps none of them and logs none of them.
 */
class SinuaVoiceModule(private val reactContext: ReactApplicationContext) :
    ReactContextBaseJavaModule(reactContext) {

    private val scope = CoroutineScope(SupervisorJob() + Dispatchers.Main)

    override fun getName() = "SinuaVoice"

    init {
        VoiceRegistry.onEvent = { id, event, payload ->
            val body = Arguments.createMap()
            body.putString("id", id)
            body.putString("event", event)
            for ((k, v) in payload) if (v is String) body.putString(k, v)
            reactContext
                .getJSModule(DeviceEventManagerModule.RCTDeviceEventEmitter::class.java)
                .emit("sinua-voice", body)
        }
    }

    /** RN's NativeEventEmitter calls these; the emitter here is the device event one. */
    @ReactMethod fun addListener(eventName: String) = Unit

    @ReactMethod fun removeListeners(count: Int) = Unit

    @ReactMethod
    fun create(config: ReadableMap, promise: Promise) {
        try {
            VoiceRegistry.create(reactContext, config)
            promise.resolve(config.getString("id"))
        } catch (e: Exception) {
            promise.reject("sinua_voice_create", e.message, e)
        }
    }

    @ReactMethod
    fun connect(id: String, promise: Promise) {
        scope.launch {
            try {
                // Sources are main-thread objects (the native contract), and connect() returns
                // once the socket is open; later failures arrive as an `error` event instead.
                withContext(Dispatchers.Main) { VoiceRegistry.connect(id) }
                promise.resolve(null)
            } catch (e: Exception) {
                promise.reject("sinua_voice_connect", e.message, e)
            }
        }
    }

    @ReactMethod
    fun disconnect(id: String) {
        scope.launch { VoiceRegistry.disconnect(id) }
    }

    @ReactMethod
    fun release(id: String) {
        scope.launch { VoiceRegistry.release(id) }
    }

    @ReactMethod
    fun provideCredential(requestId: String, credential: String?, error: String?) {
        VoiceRegistry.provideCredential(requestId, credential, error)
    }
}
