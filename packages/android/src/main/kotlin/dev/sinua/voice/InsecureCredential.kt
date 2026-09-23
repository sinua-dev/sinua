package dev.sinua.voice

/**
 * The one rule about long-lived API keys, shared by every adapter that can be
 * handed one. The Kotlin port of `packages/voice/src/insecureCredential.ts`;
 * the message text is deliberately identical on all three platforms.
 *
 * A production credential is short-lived and minted server-side: OpenAI's
 * `ek_…` (`POST /v1/realtime/client_secrets`) or Gemini's `auth_tokens/…`
 * (`POST /v1beta/auth_tokens`). A raw account key is a different object --
 * long-lived, unscoped and billable -- so it is **refused** unless the caller
 * opts in with `allowInsecureApiKey`. Prior art for the shape:
 * `openai-agents-js` refuses a raw key in a browser unless `useInsecureApiKey`
 * is set.
 *
 * Placement rules this object exists to keep consistent:
 *
 * - the check runs inside `connect()`, **never a constructor** -- the Studio
 *   builds a source outside its `try`, so a throwing constructor would take the
 *   panel down instead of showing the error inline;
 * - it runs before the microphone, the audio graph and the socket, so a refused
 *   credential never opens a device or a connection.
 */
object InsecureCredential {
    /** Thrown by [check] when a raw key is used without the opt-in. */
    class Refused(message: String) : IllegalArgumentException(message)

    const val GEMINI_SHAPE = "auth_tokens/…"
    const val GEMINI_MINT_HINT = "POST https://generativelanguage.googleapis.com/v1beta/auth_tokens"
    const val OPENAI_SHAPE = "ek_…"
    const val OPENAI_MINT_HINT = "POST https://api.openai.com/v1/realtime/client_secrets"

    /** `auth_tokens/…` is Gemini's ephemeral shape. */
    fun isGeminiEphemeral(credential: String): Boolean = credential.trim().startsWith("auth_tokens/")

    /** `ek_…` is OpenAI's ephemeral shape. */
    fun isOpenAIEphemeral(credential: String): Boolean = credential.trim().startsWith("ek_")

    /**
     * Logcat on a device; stderr under a JVM unit test, where `android.util.Log`
     * is a stub that throws. A warning must never be the thing that fails a
     * connect, so the fallback is unconditional.
     */
    private fun defaultWarn(message: String) {
        runCatching { android.util.Log.w("SinuaVoice", message) }
            .onFailure { System.err.println(message) }
    }

    /**
     * Throws [Refused] for a raw key used without the opt-in; returns normally
     * when the connect may go ahead. When a raw key *is* allowed this warns
     * once per call, so a local demo can't quietly turn into a deployment.
     *
     * [warn] is overridable so tests can observe the warning without logging.
     */
    fun check(
        vendor: String,
        isEphemeral: Boolean,
        allowInsecureApiKey: Boolean,
        ephemeralShape: String,
        mintHint: String,
        warn: (String) -> Unit = ::defaultWarn,
    ) {
        if (isEphemeral) return
        if (!allowInsecureApiKey) {
            throw Refused(
                "$vendor: refusing a raw, long-lived API key. Pass a short-lived credential " +
                    "($ephemeralShape) minted by your own backend ($mintHint). " +
                    "For a local demo only, set `allowInsecureApiKey: true`.",
            )
        }
        warn(
            "$vendor: connecting with a raw, long-lived API key because `allowInsecureApiKey` " +
                "is set. That key is exposed on the device -- this is for local demos, not for " +
                "shipping. In a product, send a $ephemeralShape minted by your backend ($mintHint).",
        )
    }
}
