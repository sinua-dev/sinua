package dev.sinua.voice

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.IOException
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * The shared credential contract ([CredentialSource]), the Kotlin port of
 * `packages/voice/src/credential.ts` and the same rules as its tests: one JSON
 * shape, a provider or URL asked on every resolve, 4xx/bad shape fatal,
 * 5xx/429 retryable, and the credential never in a message. No network.
 */
class CredentialTest {
    private object Inline : MainDispatcher {
        override fun post(r: Runnable) = r.run()
        override fun postDelayed(r: Runnable, delayMs: Long) = r.run()
        override fun remove(r: Runnable) {}
    }

    private fun CredentialSource.await(needsUrl: Boolean = false): Result<SinuaCredential> {
        var out: Result<SinuaCredential>? = null
        val done = CountDownLatch(1)
        resolve("V", Inline, needsUrl) {
            out = it
            done.countDown()
        }
        assertTrue(done.await(5, TimeUnit.SECONDS))
        return out!!
    }

    @Test
    fun valueProviderAndUrlResolveToTheSharedShape() {
        assertEquals(SinuaCredential("ek_a"), CredentialSource.fixed(" ek_a ").await().getOrThrow())
        assertFalse(CredentialSource.fixed("x").canRefresh)

        var calls = 0
        val p = CredentialSource.provider { SinuaCredential("ek_${++calls}", expiresAt = 5.0) }
        assertTrue(p.canRefresh)
        p.await()
        assertEquals("asked again on every resolve", SinuaCredential("ek_2", 5.0), p.await().getOrThrow())

        val seen = mutableListOf<String>()
        val u = CredentialSource.url("https://app.example/api/voice/livekit") { url ->
            seen += url
            200 to """{"credential":"jwt","url":"wss://lk.example","expiresAt":9}"""
        }
        assertEquals(SinuaCredential("jwt", 9.0, "wss://lk.example"), u.await(needsUrl = true).getOrThrow())
        assertEquals(listOf("https://app.example/api/voice/livekit"), seen)
    }

    @Test
    fun aFixedValueAnswersSynchronously() {
        var answered = false
        CredentialSource.fixed("ek_x").resolve(
            "V",
            object : MainDispatcher {
                override fun post(r: Runnable) = error("a fixed value must not hop threads")
                override fun postDelayed(r: Runnable, delayMs: Long) = error("no")
                override fun remove(r: Runnable) {}
            },
        ) { answered = true }
        assertTrue("so a refusal can still be thrown from connect()", answered)
    }

    @Test
    fun badShapesAreFatalAndNeverEchoTheValue() {
        for (body in listOf(
            "",
            "<html>",
            "{}",
            """{"key":"ek_SECRET"}""",
            """{"credential":1}""",
            """{"credential":"  "}""",
            """{"credential":"x","expiresAt":"soon"}""",
            """{"credential":"x","url":2}""",
        )) {
            val e = runCatching { CredentialSource.decode("V", body) }.exceptionOrNull() as? CredentialException
            assertTrue(body, e?.fatal == true)
            assertFalse("names keys, never values", e!!.message!!.contains("SECRET"))
        }
    }

    @Test
    fun httpStatusesSplitIntoFatalAndRetryable() {
        for ((status, fatal) in listOf(401 to true, 404 to true, 408 to false, 429 to false, 500 to false, 503 to false)) {
            val e = CredentialSource.url("https://a.example/c") { status to "" }.await().exceptionOrNull()
            assertEquals("$status", fatal, (e as CredentialException).fatal)
        }
        val down = CredentialSource.url("https://a.example/c") { throw IOException("offline") }.await().exceptionOrNull()
        assertFalse("network errors are retried", (down as CredentialException).fatal)
    }

    @Test
    fun liveKitNeedsAUrl() {
        val e = CredentialSource.fixed("jwt").await(needsUrl = true).exceptionOrNull() as CredentialException
        assertTrue(e.fatal)
        assertTrue(e.message!!.contains("no `url`"))
    }
}
