package dev.sinua.reactnative

import android.content.Context
import dev.sinua.voice.CredentialException
import dev.sinua.voice.CredentialSource
import dev.sinua.voice.LocalMicVoiceSource
import dev.sinua.voice.SimulatedVoiceSource
import dev.sinua.voice.SinuaCredential
import dev.sinua.voice.TestToneVoiceSource
import dev.sinua.voice.SharedVoiceSource
import dev.sinua.voice.VoiceSource
import com.facebook.react.bridge.ReadableMap
import kotlinx.coroutines.CompletableDeferred
import kotlinx.coroutines.runBlocking

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

    /** The JS side's `credential` provider / `credentialUrl`: asked again for every (re)connect. */
    fun interface CredentialProvider {
        suspend fun fetch(): SinuaCredential
    }

    /**
     * The config's credential as the shared [CredentialSource]: a JS provider or
     * `credentialUrl` (`hasCredentialProvider`) is a round trip per (re)connect;
     * otherwise the fixed `credential` string.
     */
    fun credentialSource(config: ReadableMap, credentials: CredentialProvider, missing: String): CredentialSource {
        if (config.hasKey("hasCredentialProvider") && config.getBoolean("hasCredentialProvider")) {
            // Runs on CredentialSource's background thread; the answer comes back over the bridge.
            return CredentialSource.provider { runBlocking { credentials.fetch() } }
        }
        val credential = config.getString("credential")?.takeIf { it.isNotBlank() } ?: throw VoiceError(missing)
        return CredentialSource.fixed(credential)
    }

    class VoiceError(message: String) : Exception(message)

    /**
     * Each source behind its fan-out: the JS events and every view bound by id listen side by
     * side (a source holds one callback of each kind).
     */
    private val sources = mutableMapOf<String, SharedVoiceSource>()
    private val pending = mutableMapOf<String, CompletableDeferred<SinuaCredential>>()

    /** Emitted state / error / interrupt / credentialRequest; the module forwards them to JS. */
    var onEvent: ((id: String, event: String, payload: Map<String, Any>) -> Unit)? = null

    fun source(id: String): SharedVoiceSource? = synchronized(sources) { sources[id] }

    fun create(context: Context, config: ReadableMap) {
        val id = config.getString("id") ?: throw VoiceError("voice source needs an id")
        val vendor = config.getString("vendor") ?: throw VoiceError("voice source needs a vendor")
        val source = SharedVoiceSource.of(make(context, vendor, id, config))
        source.listenState { state -> onEvent?.invoke(id, "state", mapOf("state" to state.wire)) }
        source.listenInterrupt { onEvent?.invoke(id, "interrupt", emptyMap()) }
        source.listenConnection { up -> onEvent?.invoke(id, "connection", mapOf("connected" to up)) }
        source.listenMute { muted -> onEvent?.invoke(id, "mute", mapOf("muted" to muted)) }
        // The fan-out holds the source's one onError (the vendor factory's callback is replaced),
        // so failures after connect() reach JS from here.
        source.listenError { e -> onEvent?.invoke(id, "error", mapOf("message" to (e.message ?: e.toString()))) }
        synchronized(sources) { sources[id] = source }
    }

    /** Mutes the microphone (silence goes out, the session stays up); views bound by id show the cue. */
    fun setMuted(id: String, muted: Boolean) {
        source(id)?.setMuted(muted)
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

    // MARK: credential round trip (JS resolves `credential` providers and `credentialUrl`)

    suspend fun requestCredential(id: String): SinuaCredential {
        val requestId = java.util.UUID.randomUUID().toString()
        val waiter = CompletableDeferred<SinuaCredential>()
        synchronized(pending) { pending[requestId] = waiter }
        onEvent?.invoke(id, "credentialRequest", mapOf("requestId" to requestId))
        return waiter.await()
    }

    fun provideCredential(requestId: String, credential: String?, url: String?, error: String?, fatal: Boolean) {
        val waiter = synchronized(pending) { pending.remove(requestId) } ?: return
        if (!credential.isNullOrEmpty()) {
            waiter.complete(SinuaCredential(credential, url = url?.takeIf { it.isNotEmpty() }))
        } else {
            waiter.completeExceptionally(CredentialException(error ?: "the credential provider returned nothing", fatal))
        }
    }

    // MARK: vendors

    private fun make(context: Context, vendor: String, id: String, config: ReadableMap): VoiceSource = when (vendor) {
        "test" -> TestToneVoiceSource()
        "mic" -> LocalMicVoiceSource()
        "simulated" -> {
            val loop = if (config.hasKey("loop")) config.getBoolean("loop") else null
            val script = config.getString("script")?.takeIf { it.isNotBlank() }
            try {
                if (script != null) SimulatedVoiceSource(script, loop = loop)
                else SimulatedVoiceSource.sample(config.getString("sample") ?: "calendar", loop = loop)
            } catch (e: IllegalArgumentException) {
                throw VoiceError(e.message ?: "invalid simulated conversation")
            }
        }
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
