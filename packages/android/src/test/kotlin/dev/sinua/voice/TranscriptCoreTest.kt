package dev.sinua.voice

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * The transcript rules (design note 39): spec/transcript-cases.json, the table Web and iOS run
 * too, through [OpenAILiveSession] with GPT-Live events and level ticks. Plain JVM: no
 * Android, no network, no audio.
 */
class TranscriptCoreTest {
    /** An update as the table writes it: only the keys that are set. */
    private fun describe(u: TranscriptUpdate?): String? {
        if (u == null) return null
        val parts = mutableListOf("role=${u.role.wire}", "text=${u.text}", "final=${u.final}", "turnId=${u.turnId}")
        if (u.truncated) parts += "truncated=true"
        u.startMs?.let { parts += "startMs=$it" }
        u.endMs?.let { parts += "endMs=$it" }
        return parts.joinToString(", ")
    }

    private fun describe(o: JSONObject?): String? {
        if (o == null) return null
        val parts = mutableListOf(
            "role=${o.getString("role")}",
            "text=${o.getString("text")}",
            "final=${o.getBoolean("final")}",
            "turnId=${o.getString("turnId")}",
        )
        if (o.optBoolean("truncated", false)) parts += "truncated=true"
        if (o.has("startMs")) parts += "startMs=${o.getDouble("startMs")}"
        if (o.has("endMs")) parts += "endMs=${o.getDouble("endMs")}"
        return parts.joinToString(", ")
    }

    @Test fun theAssemblerFollowsTheSharedTable() {
        val root = JSONObject(File("../../spec/transcript-cases.json").readText())
        val k = root.getJSONObject("constants")
        assertEquals(TranscriptAssembler.USER_SILENCE_MS, k.getDouble("userSilenceMs"), 0.0)
        assertEquals(TranscriptAssembler.USER_CLOSE_CHARS, k.getInt("userCloseChars"))
        assertEquals(TranscriptAssembler.REVEAL_CHARS_PER_SECOND, k.getDouble("revealCharsPerSecond"), 0.0)
        assertEquals(TranscriptAssembler.CUT_GRACE_MS, k.getDouble("cutGraceMs"), 0.0)
        assertEquals(TranscriptAssembler.SEGMENT_DELAY_MS, k.getDouble("segmentDelayMs"), 0.0)
        assertEquals(TranscriptAssembler.NEW_UTTERANCE_GAP_MS, k.getDouble("newUtteranceGapMs"), 0.0)
        assertEquals(TranscriptAssembler.AUDIBLE_LEVEL, k.getDouble("audibleLevel"), 0.0)
        val cases = root.getJSONArray("cases")
        assertTrue(cases.length() >= 12)
        for (c in 0 until cases.length()) {
            val case = cases.getJSONObject(c)
            val name = case.getString("name")
            val s = OpenAILiveSession(case.getBoolean("syncToAudio"))
            var lastUser: TranscriptUpdate? = null
            var lastAssistant: TranscriptUpdate? = null
            var finals = 0
            val finalIds = HashSet<String>()
            val shown = HashMap<String, String>()
            s.onTranscript = { u ->
                assertFalse("$name: update after ${u.turnId}'s final", u.turnId in finalIds)
                if (!u.truncated && !u.final) assertTrue("$name: ${u.turnId} shrank", u.text.startsWith(shown[u.turnId] ?: ""))
                shown[u.turnId] = u.text
                if (u.final) {
                    finalIds += u.turnId
                    finals++
                }
                if (u.role == TranscriptRole.USER) lastUser = u else lastAssistant = u
            }
            s.connecting()
            val steps = case.getJSONArray("steps")
            for (i in 0 until steps.length()) {
                val step = steps.getJSONObject(i)
                val at = "$name, step ${i + 1}"
                val t = step.getDouble("t")
                when {
                    step.has("event") -> s.handle(step.getJSONObject("event").toString(), t)

                    step.has("reconnect") -> s.connecting()

                    else -> {
                        val level = step.getDouble("level")
                        repeat(step.optInt("repeat", 1)) { n -> s.tick(level, t + n * 33) }
                    }
                }
                assertEquals("$at: user", describe(step.optJSONObject("user")), describe(lastUser))
                assertEquals("$at: assistant", describe(step.optJSONObject("assistant")), describe(lastAssistant))
                assertEquals("$at: finals", step.getInt("finals"), finals)
            }
        }
    }

    @Test fun sharedVoiceSourceFansOutTranscriptsAndUnsubscribes() {
        val fake = object : VoiceSource {
            var cb: ((TranscriptUpdate) -> Unit)? = null
            override fun connect() {}
            override fun disconnect() {}
            override fun onMetrics(cb: (VoiceMetrics) -> Unit) {}
            override fun onStateChange(cb: (AgentState) -> Unit) {}
            override fun onTranscript(cb: (TranscriptUpdate) -> Unit) {
                this.cb = cb
            }
            override val supportsTranscript: Boolean get() = true
            override val transcriptTiming: TranscriptTiming get() = TranscriptTiming.SEGMENTS
        }
        val shared = SharedVoiceSource.of(fake)
        val a = mutableListOf<String>()
        val b = mutableListOf<String>()
        val offA = shared.listenTranscript { a += it.text }
        shared.listenTranscript { b += it.text }
        assertTrue(shared.supportsTranscript)
        assertEquals(TranscriptTiming.SEGMENTS, shared.transcriptTiming)
        fake.cb?.invoke(TranscriptUpdate(TranscriptRole.USER, "Merhaba", false, "u1"))
        offA()
        fake.cb?.invoke(TranscriptUpdate(TranscriptRole.USER, "Merhaba!", true, "u1"))
        assertEquals(listOf("Merhaba"), a)
        assertEquals(listOf("Merhaba", "Merhaba!"), b)
    }
}
