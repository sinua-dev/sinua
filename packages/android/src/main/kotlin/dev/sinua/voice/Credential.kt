package dev.sinua.voice

import org.json.JSONException
import org.json.JSONObject
import java.io.IOException
import java.net.HttpURLConnection
import java.net.URL
import java.util.concurrent.Executors

/**
 * The one credential contract every vendor source takes, on every platform --
 * the Kotlin port of `packages/voice/src/credential.ts` (docs/audio-pipeline.md,
 * *Credentials for a real integration*).
 *
 * Your backend mints a short-lived credential and answers with this JSON, the
 * same shape for OpenAI (`ek_…`), Gemini (`auth_tokens/…`), ElevenLabs (a signed
 * `wss://` URL) and LiveKit (a room JWT plus the server `url`):
 * `{ "credential": "…", "expiresAt": 1790000000, "url": "wss://…" }`.
 */
data class SinuaCredential(
    /** The short-lived credential itself. */
    val credential: String,
    /** When it stops working, in Unix seconds, if the vendor says. */
    val expiresAt: Double? = null,
    /** LiveKit only: the server URL the token is for. */
    val url: String? = null,
)

/**
 * A credential failure. [fatal]: a retry can't fix it (a 4xx, a bad shape, a
 * refused raw key); otherwise it's a network error, a 429 or a 5xx. The
 * credential itself never appears in the message.
 */
class CredentialException(message: String, val fatal: Boolean) : IllegalArgumentException(message)

/** How a `credentialUrl` is fetched: POST `url` -> (HTTP status, body). Tests pass a fake. */
fun interface CredentialHttp {
    @Throws(IOException::class)
    fun post(url: String): Pair<Int, String>
}

/** `java.net.HttpURLConnection`: no dependency, no cache, `Accept: application/json`. */
class HttpUrlCredentialHttp(private val timeoutMs: Int = 15_000) : CredentialHttp {
    override fun post(url: String): Pair<Int, String> {
        val c = URL(url).openConnection() as HttpURLConnection
        try {
            c.requestMethod = "POST"
            c.useCaches = false
            c.connectTimeout = timeoutMs
            c.readTimeout = timeoutMs
            c.doOutput = true
            c.setRequestProperty("Accept", "application/json")
            c.setFixedLengthStreamingMode(0)
            c.outputStream.close()
            val status = c.responseCode
            val stream = if (status in 200..299) c.inputStream else c.errorStream
            return status to (stream?.bufferedReader()?.use { it.readText() } ?: "")
        } finally {
            c.disconnect()
        }
    }
}

/**
 * Where a source gets its credential. Asked again on **every** connect and
 * reconnect, so an expired or spent credential is never reused.
 *
 * Callback-based (the voice core has no coroutines): [fixed] answers
 * synchronously, so a refusal still surfaces from `connect()` itself; [provider]
 * and [url] run on a background thread and answer on the source's main
 * dispatcher.
 */
class CredentialSource private constructor(
    /** True when a reconnect can get a *new* credential (a provider or a URL). */
    val canRefresh: Boolean,
    private val fixedValue: String?,
    private val fetch: ((String) -> SinuaCredential)?,
) {
    /**
     * The credential for this (re)connect, delivered to [cb] on [main] (or, for a
     * fixed value, right away on the calling thread). [needsUrl]: LiveKit.
     */
    fun resolve(
        vendor: String,
        main: MainDispatcher,
        needsUrl: Boolean = false,
        cb: (Result<SinuaCredential>) -> Unit,
    ) {
        val f = fetch
        if (f == null) {
            cb(runCatching { checkUrl(vendor, parse(vendor, fixedValue ?: ""), needsUrl) })
            return
        }
        background.execute {
            val r = runCatching { checkUrl(vendor, f(vendor), needsUrl) }
            main.post { cb(r) }
        }
    }

    /** Blocking resolve, for code that already runs off the main thread (tests, factories). */
    fun resolveBlocking(vendor: String, needsUrl: Boolean = false): SinuaCredential =
        checkUrl(vendor, fetch?.invoke(vendor) ?: parse(vendor, fixedValue ?: ""), needsUrl)

    companion object {
        private val background = Executors.newCachedThreadPool { r ->
            Thread(r, "sinua-credential").apply { isDaemon = true }
        }

        /** One fixed value: a credential pasted for a single session, or ElevenLabs' public agent id. */
        @JvmStatic
        fun fixed(credential: String) = CredentialSource(false, credential, null)

        /**
         * The production shape: [provide] fetches a fresh credential from your backend.
         * It runs on a background thread, so blocking I/O is fine there.
         */
        @JvmStatic
        fun provider(provide: () -> SinuaCredential) = CredentialSource(true, null) { vendor ->
            val c = try {
                provide()
            } catch (e: CredentialException) {
                throw e
            } catch (e: IOException) {
                throw CredentialException("$vendor: the credential provider failed: ${e.message}", fatal = false)
            }
            parse(vendor, c.credential, c.expiresAt, c.url)
        }

        /**
         * Shorthand for a provider that `POST`s to your endpoint (no body, no
         * cache) and reads a [SinuaCredential] back -- the same on every platform.
         */
        @JvmStatic
        @JvmOverloads
        fun url(url: String, http: CredentialHttp = HttpUrlCredentialHttp()) = CredentialSource(true, null) { vendor ->
            val (status, body) = try {
                http.post(url)
            } catch (e: IOException) {
                throw CredentialException("$vendor: $url failed: ${e.message}", fatal = false)
            }
            if (status !in 200..299) {
                throw CredentialException("$vendor: $url returned $status", fatal = !isRetryableHttpStatus(status))
            }
            decode(vendor, body)
        }

        /** Validates an endpoint's JSON answer against [SinuaCredential]. */
        @JvmStatic
        fun decode(vendor: String, body: String): SinuaCredential {
            val o = try {
                JSONObject(body)
            } catch (e: JSONException) {
                throw CredentialException("$vendor: the credential endpoint did not return JSON", fatal = true)
            }
            val c = o.opt("credential")
            if (c !is String) {
                val keys = o.keys().asSequence().sorted().joinToString(", ")
                throw CredentialException(
                    "$vendor: expected { credential: string, expiresAt?, url? }, got keys [$keys]",
                    true,
                )
            }
            val e = o.opt("expiresAt")
            val expiresAt = when {
                e == null || e == JSONObject.NULL -> null
                e is Number -> e.toDouble()
                else -> throw CredentialException("$vendor: `expiresAt` must be Unix seconds (a number)", true)
            }
            val u = o.opt("url")
            val url = when {
                u == null || u == JSONObject.NULL -> null
                u is String -> u
                else -> throw CredentialException("$vendor: `url` must be a string", true)
            }
            return parse(vendor, c, expiresAt, url)
        }

        internal fun parse(
            vendor: String,
            credential: String,
            expiresAt: Double? = null,
            url: String? = null,
        ): SinuaCredential {
            val c = credential.trim()
            if (c.isEmpty()) throw CredentialException("$vendor: a credential is required", fatal = true)
            return SinuaCredential(c, expiresAt, url)
        }

        private fun checkUrl(vendor: String, c: SinuaCredential, needsUrl: Boolean): SinuaCredential {
            if (needsUrl && c.url.isNullOrEmpty()) {
                throw CredentialException(
                    "$vendor: the credential has no `url` (LiveKit needs { credential, url })",
                    true,
                )
            }
            return c
        }

        /** Timeouts, rate limits and server errors are worth retrying; any other 4xx isn't. */
        internal fun isRetryableHttpStatus(status: Int) =
            status == 408 || status == 425 || status == 429 || status >= 500
    }
}
