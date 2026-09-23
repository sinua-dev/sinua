package dev.sinua.voice

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

/**
 * The raw-API-key gate ([InsecureCredential]). The same rule and the same
 * message text exist on Web (`packages/voice/test/sources.test.mjs`) and iOS
 * (`InsecureCredentialTests.swift`); these assert the shared shape so the three
 * can't drift apart silently.
 */
class InsecureCredentialTest {
    private fun refusal(body: () -> Unit): String? = try {
        body()
        null
    } catch (e: InsecureCredential.Refused) {
        e.message
    }

    @Test
    fun ephemeralCredentialPassesAndWarnsAboutNothing() {
        val warnings = mutableListOf<String>()
        assertEquals(
            null,
            refusal {
                InsecureCredential.check(
                    vendor = "GeminiLiveVoiceSource",
                    isEphemeral = true,
                    allowInsecureApiKey = false,
                    ephemeralShape = InsecureCredential.GEMINI_SHAPE,
                    mintHint = InsecureCredential.GEMINI_MINT_HINT,
                    warn = { warnings += it },
                )
            },
        )
        assertTrue(warnings.isEmpty())
    }

    @Test
    fun rawKeyIsRefusedWithoutTheOptIn() {
        val text = refusal {
            InsecureCredential.check(
                vendor = "GeminiLiveVoiceSource",
                isEphemeral = false,
                allowInsecureApiKey = false,
                ephemeralShape = InsecureCredential.GEMINI_SHAPE,
                mintHint = InsecureCredential.GEMINI_MINT_HINT,
                warn = { fail("a refusal must not also warn") },
            )
        } ?: return fail("expected a refusal")
        assertTrue(text, text.contains("refusing a raw, long-lived API key"))
        assertTrue("says what to pass instead: $text", text.contains("auth_tokens/…"))
        assertTrue("says how to mint one: $text", text.contains("v1beta/auth_tokens"))
        assertTrue("says how to opt in: $text", text.contains("allowInsecureApiKey"))
    }

    @Test
    fun theOptInAllowsItAndWarnsExactlyOnce() {
        val warnings = mutableListOf<String>()
        assertEquals(
            null,
            refusal {
                InsecureCredential.check(
                    vendor = "OpenAIRealtimeVoiceSource",
                    isEphemeral = false,
                    allowInsecureApiKey = true,
                    ephemeralShape = InsecureCredential.OPENAI_SHAPE,
                    mintHint = InsecureCredential.OPENAI_MINT_HINT,
                    warn = { warnings += it },
                )
            },
        )
        assertEquals(1, warnings.size)
        assertTrue(warnings[0], warnings[0].contains("local demos, not for shipping"))
    }

    @Test
    fun credentialShapes() {
        assertTrue(InsecureCredential.isGeminiEphemeral("auth_tokens/abc"))
        assertTrue(InsecureCredential.isGeminiEphemeral("  auth_tokens/abc  "))
        assertFalse(InsecureCredential.isGeminiEphemeral("AIzaKEY"))
        assertTrue(InsecureCredential.isOpenAIEphemeral("ek_abc"))
        assertFalse(InsecureCredential.isOpenAIEphemeral("sk-abc"))
    }
}
