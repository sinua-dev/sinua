package dev.sinua.reactnative.vendors

import android.content.Context
import dev.sinua.livekit.LiveKitVoiceSource
import dev.sinua.reactnative.VoiceRegistry
import dev.sinua.voice.VoiceSource
import com.facebook.react.bridge.ReadableMap

/**
 * `{ vendor: "livekit", url, token, publishMicrophone? }` or
 * `{ vendor: "livekit", credential | credentialUrl, publishMicrophone? }` (src/voice.ts).
 * Compiled only when `sinua.voiceVendors` contains `livekit` (it pulls livekit-android).
 */
class LivekitFactory : VoiceRegistry.VendorFactory {
    override fun create(
        context: Context,
        config: ReadableMap,
        credentials: VoiceRegistry.CredentialProvider,
        errors: (String) -> Unit,
    ): VoiceSource {
        val publish = if (config.hasKey("publishMicrophone")) config.getBoolean("publishMicrophone") else true
        val url = config.getString("url")?.takeIf { it.isNotBlank() }
        val token = config.getString("token")?.takeIf { it.isNotBlank() }
        val source = if (url != null && token != null) {
            LiveKitVoiceSource.owned(context, url, token, publish)
        } else {
            val needs = "livekit needs a url and a token, or credentialUrl / a credential provider answering { credential, url }"
            if (!(config.hasKey("hasCredentialProvider") && config.getBoolean("hasCredentialProvider"))) {
                throw VoiceRegistry.VoiceError(needs)
            }
            LiveKitVoiceSource.owned(context, VoiceRegistry.credentialSource(config, credentials, needs), publish)
        }
        source.onError { errors(it.message ?: it.toString()) }
        return source
    }
}
