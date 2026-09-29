package dev.sinua.reactnative.vendors

import android.content.Context
import dev.sinua.gemini.GeminiLiveVoiceSource
import dev.sinua.reactnative.VoiceRegistry
import dev.sinua.voice.GeminiLiveSession
import dev.sinua.voice.VoiceSource
import com.facebook.react.bridge.ReadableMap

/** `{ vendor: "gemini", credential | credentialUrl, model?, instructions? (deprecated), endpoint? }` (src/voice.ts). */
class GeminiFactory : VoiceRegistry.VendorFactory {
    override fun create(
        context: Context,
        config: ReadableMap,
        credentials: VoiceRegistry.CredentialProvider,
        errors: (String) -> Unit,
    ): VoiceSource {
        val source = GeminiLiveVoiceSource(
            VoiceRegistry.credentialSource(
                config,
                credentials,
                "gemini needs a credential or credentialUrl (an ephemeral auth_tokens/… from your backend)",
            ),
            model = config.getString("model")?.takeIf { it.isNotBlank() } ?: GeminiLiveSession.DEFAULT_MODEL,
            instructions = config.getString("instructions")?.takeIf { it.isNotBlank() },
            endpointOverride = config.getString("endpoint")?.takeIf { it.isNotBlank() }?.let { GeminiLiveSession.Endpoint(it, emptyMap()) },
        )
        source.onError { errors(it.message ?: it.toString()) }
        return source
    }
}
