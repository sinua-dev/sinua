package dev.sinua.reactnative.vendors

import android.content.Context
import dev.sinua.openai.OpenAIRealtimeVoiceSource
import dev.sinua.reactnative.VoiceRegistry
import dev.sinua.voice.VoiceSource
import com.facebook.react.bridge.ReadableMap

/**
 * `{ vendor: "openai", credential | credentialUrl }` (src/voice.ts).
 * Compiled only when `sinua.voiceVendors` contains `openai` (it pulls the WebRTC binary).
 *
 * With a provider or `credentialUrl`, every (re)connect asks JS for a fresh `ek_`,
 * which a single-use client secret needs.
 */
class OpenaiFactory : VoiceRegistry.VendorFactory {
    override fun create(
        context: Context,
        config: ReadableMap,
        credentials: VoiceRegistry.CredentialProvider,
        errors: (String) -> Unit,
    ): VoiceSource {
        val source = OpenAIRealtimeVoiceSource(
            context,
            VoiceRegistry.credentialSource(config, credentials, "openai needs a credential or credentialUrl (an ek_… from your backend)"),
            transcribeUser = if (config.hasKey("transcribeUser")) config.getString("transcribeUser") else null,
        )
        source.onError { errors(it.message ?: it.toString()) }
        return source
    }
}
