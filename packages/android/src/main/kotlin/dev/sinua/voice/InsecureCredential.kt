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
}
