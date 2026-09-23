package dev.sinua.reactnative.vendors

import android.content.Context
import dev.sinua.openai.OpenAIRealtimeVoiceSource
import dev.sinua.reactnative.VoiceRegistry
import dev.sinua.voice.OpenAIRealtimeSignaling
import dev.sinua.voice.VoiceSource
import com.facebook.react.bridge.ReadableMap

/**
 * `{ vendor: "openai", credential | getCredential, model?, voice?, instructions?, allowInsecureApiKey? }` (src/voice.ts).
 * Compiled only when `sinua.voiceVendors` contains `openai` (it pulls the WebRTC binary).
 *
 * With `getCredential`, every (re)connect asks JS for a fresh `ek_`, which a
 * single-use client secret needs.
 */
class OpenaiFactory : VoiceRegistry.VendorFactory {
    override fun create(
        context: Context,
        config: ReadableMap,
        credentials: VoiceRegistry.CredentialProvider,
        errors: (String) -> Unit,
    ): VoiceSource {
        val source = if (config.hasKey("hasCredentialProvider") && config.getBoolean("hasCredentialProvider")) {
            OpenAIRealtimeVoiceSource(context, credentialProvider = { credentials.fetch() })
        } else {
            val credential = config.getString("credential")?.takeIf { it.isNotBlank() }
                ?: throw VoiceRegistry.VoiceError("openai needs a credential (an ek_… from your backend) or getCredential")
            OpenAIRealtimeVoiceSource.withCredential(
                context,
                credential,
                model = config.getString("model")?.takeIf { it.isNotBlank() } ?: OpenAIRealtimeSignaling.DEFAULT_MODEL,
                voice = config.getString("voice")?.takeIf { it.isNotBlank() } ?: OpenAIRealtimeSignaling.DEFAULT_VOICE,
                instructions = config.getString("instructions")?.takeIf { it.isNotBlank() },
                allowInsecureApiKey = config.hasKey("allowInsecureApiKey") && config.getBoolean("allowInsecureApiKey"),
            )
        }
        source.onError { errors(it.message ?: it.toString()) }
        return source
    }
}
