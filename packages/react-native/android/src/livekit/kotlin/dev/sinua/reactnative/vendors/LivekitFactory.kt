package dev.sinua.reactnative.vendors

import android.content.Context
import dev.sinua.livekit.LiveKitVoiceSource
import dev.sinua.reactnative.VoiceRegistry
import dev.sinua.voice.VoiceSource
import com.facebook.react.bridge.ReadableMap

/**
 * `{ vendor: "livekit", url, token, publishMicrophone? }` (src/voice.ts).
 * Compiled only when `sinua.voiceVendors` contains `livekit` (it pulls livekit-android).
 */
class LivekitFactory : VoiceRegistry.VendorFactory {
    override fun create(
        context: Context,
        config: ReadableMap,
        credentials: VoiceRegistry.CredentialProvider,
        errors: (String) -> Unit,
    ): VoiceSource {
        val url = config.getString("url")?.takeIf { it.isNotBlank() }
        val token = config.getString("token")?.takeIf { it.isNotBlank() }
        if (url == null || token == null) {
            throw VoiceRegistry.VoiceError("livekit needs a url and a token (an access token from your backend)")
        }
        val publish = if (config.hasKey("publishMicrophone")) config.getBoolean("publishMicrophone") else true
        val source = LiveKitVoiceSource.owned(context, url, token, publish)
        source.onError { errors(it.message ?: it.toString()) }
        return source
    }
}
