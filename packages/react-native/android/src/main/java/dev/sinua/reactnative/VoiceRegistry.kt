package dev.sinua.reactnative

import android.content.Context
import dev.sinua.voice.LocalMicVoiceSource
import dev.sinua.voice.TestToneVoiceSource
import dev.sinua.voice.VoiceSource
import com.facebook.react.bridge.ReadableMap
import kotlinx.coroutines.CompletableDeferred

/**
 * The native voice sources a React Native app can create (src/voice.ts), kept by
 * id so a view can bind one with the `voiceSourceId` prop and several views can
 * share it. The app owns the lifecycle: nothing here connects on its own.
 *
 * Vendors are opt-in per build: `sinua.voiceVendors` in the app's
 * gradle.properties selects which vendor sources and dependencies are compiled in
 * (default `gemini,elevenlabs`, which need nothing beyond OkHttp). A vendor that
 * isn't in the build has no factory class, and `create` says how to add it --
 * nothing is linked silently.
 *
 * Credentials live only in the source they are handed to: never stored, never logged.
 */
object VoiceRegistry {
    /** Implemented once per vendor, in that vendor's source set (src/<vendor>/kotlin). */
    interface VendorFactory {
        /**
         * [errors] reports failures that happen *after* connect() returned (a dropped
         * socket, a vendor error frame); the JS handle surfaces them as `error` events.
         */
        fun create(context: Context, config: ReadableMap, credentials: CredentialProvider, errors: (String) -> Unit): VoiceSource
    }

    /** OpenAI's `getCredential`: asked again for every (re)connect, since an `ek_` is single-use. */
    fun interface CredentialProvider {
        suspend fun fetch(): String
    }

    class VoiceError(message: String) : Exception(message)

    private val sources = mutableMapOf<String, VoiceSource>()
    private val pending = mutableMapOf<String, CompletableDeferred<String>>()

    /** Emitted state / error / interrupt / credentialRequest; the module forwards them to JS. */
    var onEvent: ((id: String, event: String, payload: Map<String, Any>) -> Unit)? = null

    fun source(id: String): VoiceSource? = synchronized(sources) { sources[id] }

    fun create(context: Context, config: ReadableMap) {
        val id = config.getString("id") ?: throw VoiceError("voice source needs an id")
        val vendor = config.getString("vendor") ?: throw VoiceError("voice source needs a vendor")
        val source = make(context, vendor, id, config)
        source.onStateChange { state -> onEvent?.invoke(id, "state", mapOf("state" to state.wire)) }
        source.onInterrupt { onEvent?.invoke(id, "interrupt", emptyMap()) }
        synchronized(sources) { sources[id] = source }
    }

    fun connect(id: String) {
        (source(id) ?: throw VoiceError("no voice source $id (already released?)")).connect()
    }

    fun disconnect(id: String) {
        source(id)?.disconnect()
    }

    fun release(id: String) {
        synchronized(sources) { sources.remove(id) }?.disconnect()
    }

    // MARK: getCredential round trip

    suspend fun requestCredential(id: String): String {
        val requestId = java.util.UUID.randomUUID().toString()
        val waiter = CompletableDeferred<String>()
        synchronized(pending) { pending[requestId] = waiter }
        onEvent?.invoke(id, "credentialRequest", mapOf("requestId" to requestId))
        return waiter.await()
    }

    fun provideCredential(requestId: String, credential: String?, error: String?) {
        val waiter = synchronized(pending) { pending.remove(requestId) } ?: return
        if (!credential.isNullOrEmpty()) waiter.complete(credential)
        else waiter.completeExceptionally(VoiceError(error ?: "getCredential returned no credential"))
    }

    // MARK: vendors

    private fun make(context: Context, vendor: String, id: String, config: ReadableMap): VoiceSource = when (vendor) {
        "test" -> TestToneVoiceSource()
        "mic" -> LocalMicVoiceSource()
        else -> factory(vendor).create(context, config, CredentialProvider { requestCredential(id) }) { message ->
            onEvent?.invoke(id, "error", mapOf("message" to message))
        }
    }

    private fun factory(vendor: String): VendorFactory {
        val className = "dev.sinua.reactnative.vendors." + vendor.replaceFirstChar { it.uppercase() } + "Factory"
        val cls = try {
            Class.forName(className)
        } catch (_: ClassNotFoundException) {
            throw VoiceError(
                "the $vendor voice source isn't in this build: add it to `sinua.voiceVendors` " +
                    "in your app's gradle.properties (e.g. sinua.voiceVendors=gemini,elevenlabs,$vendor) and rebuild",
            )
        }
        return cls.getDeclaredConstructor().newInstance() as VendorFactory
    }
}
