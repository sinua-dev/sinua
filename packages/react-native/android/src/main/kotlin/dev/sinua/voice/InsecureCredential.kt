package dev.sinua.voice

/**
 * The one rule about long-lived API keys, shared by every adapter that could be
 * handed one: **a raw key is always refused.** The Kotlin port of
 * `packages/voice/src/insecureCredential.ts`; the message text is identical on
 * all three platforms.
 *
 * A production credential is short-lived and minted server-side: OpenAI's
 * `ek_…` (`POST /v1/realtime/client_secrets`) or Gemini's `auth_tokens/…`
 * (`POST /v1beta/auth_tokens`). A raw account key is long-lived, unscoped and
 * billable. The `allowInsecureApiKey` opt-in is gone: `npx @sinua/voice
 * dev-proxy` mints real short-lived credentials on localhost, and
 * `@sinua/voice/server` does it in a backend.
 *
 * Placement: the check runs inside `connect()` (never a constructor -- the
 * Studio builds a source outside its `try`), on every credential a source
 * resolves, before the microphone, the audio graph and the socket.
 */
object InsecureCredential {
    const val GEMINI_SHAPE = "auth_tokens/…"
    const val OPENAI_SHAPE = "ek_…"

    /** `auth_tokens/…` is Gemini's ephemeral shape. */
    fun isGeminiEphemeral(credential: String): Boolean = credential.trim().startsWith("auth_tokens/")

    /** `ek_…` is OpenAI's ephemeral shape. */
    fun isOpenAIEphemeral(credential: String): Boolean = credential.trim().startsWith("ek_")

    /** The refusal message for anything that isn't the short-lived shape, or null. */
    fun refusal(vendor: String, isEphemeral: Boolean, ephemeralShape: String): String? = if (isEphemeral) {
        null
    } else {
        "$vendor: expected a short-lived credential ($ephemeralShape); refusing what looks like a raw, " +
            "long-lived API key. Mint one in your backend with @sinua/voice/server, or run " +
            "`npx @sinua/voice dev-proxy` and pass `credentialUrl`."
    }

    /** Throws a fatal [CredentialException] for anything that isn't the short-lived shape. */
    fun check(vendor: String, isEphemeral: Boolean, ephemeralShape: String) {
        refusal(vendor, isEphemeral, ephemeralShape)?.let { throw CredentialException(it, fatal = true) }
    }

    /** OpenAI's own API host: a credential sent here must be an `ek_`. */
    const val OPENAI_HOST = "api.openai.com"

    /** True when [url] is on OpenAI's own API host (an unparseable URL counts as OpenAI: the strict rule). */
    fun isOpenAIHost(url: String): Boolean {
        val host = runCatching { java.net.URI(url).host }.getOrNull() ?: return true
        return host == OPENAI_HOST
    }

    /**
     * OpenAI Realtime's rule, by where the credential goes: to OpenAI it must be an `ek_…`; to
     * your own calls endpoint (the "sideband" setup, which opens the session server-side) it's
     * your own short-lived token, any shape except a raw `sk-…` key. The same rule and message
     * as `openAICredentialRefusal` on the Web and iOS.
     */
    fun openAIRefusal(credential: String, callsUrl: String, vendor: String = "OpenAIRealtimeVoiceSource"): String? {
        val c = credential.trim()
        if (isOpenAIHost(callsUrl)) return refusal(vendor, c.startsWith("ek_"), OPENAI_SHAPE)
        if (c.startsWith("sk-")) {
            return "$vendor: your own calls endpoint takes your own short-lived token, never a raw OpenAI key " +
                "(sk-…); keep the key in your backend."
        }
        return null
    }

    /** Throws a fatal [CredentialException] when [openAIRefusal] refuses. */
    fun checkOpenAI(credential: String, callsUrl: String, vendor: String = "OpenAIRealtimeVoiceSource") {
        openAIRefusal(credential, callsUrl, vendor)?.let { throw CredentialException(it, fatal = true) }
    }

    /**
     * OpenAI GPT-Live's rule: the session is only ever opened by your server (there is no
     * `ek_`), so the session URL must be your own endpoint, never `api.openai.com`, and a
     * credential for it (optional) is your own token, never a raw `sk-…` key. The same rule
     * and messages as `openAILiveRefusal` on the Web and iOS.
     */
    fun openAILiveRefusal(sessionUrl: String, credential: String?, vendor: String = "OpenAILiveVoiceSource"): String? {
        if (isOpenAIHost(sessionUrl)) {
            return "$vendor: sessionUrl must be your own endpoint; GPT-Live sessions are opened by your server " +
                "with its key (POST /v1/live/sessions), never from the app."
        }
        if (credential?.trim()?.startsWith("sk-") == true) {
            return "$vendor: your session endpoint takes your own short-lived token, never a raw OpenAI key " +
                "(sk-…); keep the key in your backend."
        }
        return null
    }

    /** Throws a fatal [CredentialException] when [openAILiveRefusal] refuses. */
    fun checkOpenAILive(sessionUrl: String, credential: String?, vendor: String = "OpenAILiveVoiceSource") {
        openAILiveRefusal(sessionUrl, credential, vendor)?.let { throw CredentialException(it, fatal = true) }
    }
}
