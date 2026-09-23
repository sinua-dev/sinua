package dev.sinua.reactnative.vendors

import android.content.Context
import dev.sinua.elevenlabs.ElevenLabsVoiceSource
import dev.sinua.reactnative.VoiceRegistry
import dev.sinua.voice.VoiceSource
import com.facebook.react.bridge.ReadableMap

/** `{ vendor: "elevenlabs", credential, endpoint? }` (src/voice.ts). */
class ElevenlabsFactory : VoiceRegistry.VendorFactory {
    override fun create(
        context: Context,
        config: ReadableMap,
        credentials: VoiceRegistry.CredentialProvider,
        errors: (String) -> Unit,
    ): VoiceSource {
        val credential = config.getString("credential")?.takeIf { it.isNotBlank() }
            ?: throw VoiceRegistry.VoiceError("elevenlabs needs an agent id, or a signed wss:// URL from your backend")
        val source = ElevenLabsVoiceSource(
            credential = credential,
            endpoint = config.getString("endpoint")?.takeIf { it.isNotBlank() },
        )
        source.onError { errors(it.message ?: it.toString()) }
        return source
    }
}
