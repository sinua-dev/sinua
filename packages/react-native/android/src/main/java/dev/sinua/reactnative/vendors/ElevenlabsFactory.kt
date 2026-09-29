package dev.sinua.reactnative.vendors

import android.content.Context
import dev.sinua.elevenlabs.ElevenLabsVoiceSource
import dev.sinua.reactnative.VoiceRegistry
import dev.sinua.voice.VoiceSource
import com.facebook.react.bridge.ReadableMap

/** `{ vendor: "elevenlabs", credential | credentialUrl, endpoint? }` (src/voice.ts). */
class ElevenlabsFactory : VoiceRegistry.VendorFactory {
    override fun create(
        context: Context,
        config: ReadableMap,
        credentials: VoiceRegistry.CredentialProvider,
        errors: (String) -> Unit,
    ): VoiceSource {
        val source = ElevenLabsVoiceSource(
            VoiceRegistry.credentialSource(
                config,
                credentials,
                "elevenlabs needs an agent id, a signed wss:// URL, or credentialUrl",
            ),
            endpoint = config.getString("endpoint")?.takeIf { it.isNotBlank() },
        )
        source.onError { errors(it.message ?: it.toString()) }
        return source
    }
}
