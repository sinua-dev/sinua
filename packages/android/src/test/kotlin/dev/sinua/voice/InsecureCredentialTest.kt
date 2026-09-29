package dev.sinua.voice

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The raw-API-key rule ([InsecureCredential]): always refused, no opt-in. The
 * same rule and the same message text exist on Web
 * (`packages/voice/src/insecureCredential.ts`) and iOS
 * (`InsecureCredentialTests.swift`); this pins the exact text so the three can't drift.
 */
class InsecureCredentialTest {
    @Test
    fun ephemeralCredentialPasses() {
        assertNull(InsecureCredential.refusal("GeminiLiveVoiceSource", true, InsecureCredential.GEMINI_SHAPE))
        InsecureCredential.check("OpenAIRealtimeVoiceSource", true, InsecureCredential.OPENAI_SHAPE)
    }

    @Test
    fun rawKeyIsAlwaysRefusedWithTheWebMessage() {
        assertEquals(
            "GeminiLiveVoiceSource: expected a short-lived credential (auth_tokens/…); refusing what looks like a raw, " +
                "long-lived API key. Mint one in your backend with @sinua/voice/server, or run " +
                "`npx @sinua/voice dev-proxy` and pass `credentialUrl`.",
            InsecureCredential.refusal("GeminiLiveVoiceSource", false, InsecureCredential.GEMINI_SHAPE),
        )
        val e = runCatching { InsecureCredential.check("OpenAIRealtimeVoiceSource", false, InsecureCredential.OPENAI_SHAPE) }
            .exceptionOrNull()
        assertTrue("a refusal is never retried", (e as CredentialException).fatal)
    }

    @Test
    fun credentialShapes() {
        assertTrue(InsecureCredential.isGeminiEphemeral("auth_tokens/abc"))
        assertTrue("trims, as the sources do", InsecureCredential.isGeminiEphemeral("  auth_tokens/abc  "))
        assertFalse(InsecureCredential.isGeminiEphemeral("AIzaKEY"))
        assertTrue(InsecureCredential.isOpenAIEphemeral("ek_abc"))
        assertFalse(InsecureCredential.isOpenAIEphemeral("sk-abc"))
    }
}
