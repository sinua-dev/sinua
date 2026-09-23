package dev.sinua.reactnative.vendors

import android.content.Context
import dev.sinua.gemini.GeminiLiveVoiceSource
import dev.sinua.reactnative.VoiceRegistry
import dev.sinua.voice.GeminiLiveSession
import dev.sinua.voice.VoiceSource
import com.facebook.react.bridge.ReadableMap

/** `{ vendor: "gemini", credential, model?, instructions?, endpoint?, allowInsecureApiKey? }` (src/voice.ts). */
class GeminiFactory : VoiceRegistry.VendorFactory {
    override fun create(
        context: Context,
        config: ReadableMap,
        credentials: VoiceRegistry.CredentialProvider,
        errors: (String) -> Unit,
    ): VoiceSource {
        val credential = config.getString("credential")?.takeIf { it.isNotBlank() }
            ?: throw VoiceRegistry.VoiceError("gemini needs a credential (an ephemeral auth_tokens/… from your backend)")
        val source = GeminiLiveVoiceSource(
            credential = credential,
            model = config.getString("model")?.takeIf { it.isNotBlank() } ?: GeminiLiveSession.DEFAULT_MODEL,
            instructions = config.getString("instructions")?.takeIf { it.isNotBlank() },
            allowInsecureApiKey = config.hasKey("allowInsecureApiKey") && config.getBoolean("allowInsecureApiKey"),
            endpoint = config.getString("endpoint")?.takeIf { it.isNotBlank() }?.let { GeminiLiveSession.Endpoint(it, emptyMap()) },
        )
        source.onError { errors(it.message ?: it.toString()) }
        return source
    }
}
