package dev.sinua.voice

import org.json.JSONArray
import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertTrue
import org.junit.Test
import java.util.Base64

/**
 * T1b (design note 39): each vendor session feeds its transcript. The rules themselves are the
 * shared tables in TranscriptCoreTest; these check the wiring, with the clock passed in. Plain
 * JVM: no Android, no network, no audio.
 */
class VendorTranscriptTest {
    private val line = "Bir zamanlar bir sincap yaşarmış."

    /** [n] samples of 16-bit PCM, base64. */
    private fun pcm(n: Int): String = Base64.getEncoder().encodeToString(ByteArray(n * 2))

    @Test fun elevenLabsRevealsTheAlignmentAndCutsAtTheHeardCharacter() {
        val s = ElevenLabsSession()
        val got = mutableListOf<TranscriptUpdate>()
        s.onTranscript = { got += it }
        s.connecting(0.0)
        s.graphStarted(Pcm.AudioFormat(Pcm.AudioFormat.Codec.PCM, 16000), 0.0)
        s.handle(
            """{"type":"user_transcript","user_transcription_event":{"user_transcript":"Bir hikaye anlat"}}""",
            PlaybackState.DRAINED,
            0.1,
        )
        assertEquals(TranscriptUpdate(TranscriptRole.USER, "Bir hikaye anlat", true, "u1"), got.last())
        val chars = line.map { it.toString() }
        val alignment = JSONObject()
            .put("chars", JSONArray(chars))
            .put("char_start_times_ms", JSONArray(chars.indices.map { it * 60 }))
            .put("char_durations_ms", JSONArray(chars.map { 60 }))
        val audio = JSONObject().put("type", "audio").put(
            "audio_event",
            JSONObject().put("audio_base_64", pcm(32000)).put("event_id", 2).put("alignment", alignment),
        )
        s.handle(audio.toString(), PlaybackState.DRAINED, 1.0)
        var t = 1.0
        while (t <= 1.6) {
            s.tick(PlaybackState.AUDIBLE, t, 0.5)
            t += 1.0 / 30
        }
        val mid = got.last { it.role == TranscriptRole.ASSISTANT }
        assertTrue(mid.text, line.startsWith(mid.text) && mid.text.length in 7..11 && !mid.final)
        s.handle("""{"type":"interruption","interruption_event":{"event_id":3}}""", PlaybackState.AUDIBLE, 1.65)
        val cut = got.last()
        assertTrue(cut.final && cut.truncated)
        assertTrue(cut.text, line.startsWith(cut.text) && cut.text.length < line.length)
    }

    @Test fun geminiAsksForBothTranscriptionsAndPacesTheReplyOverItsAudio() {
        val s = GeminiLiveSession("m")
        val got = mutableListOf<TranscriptUpdate>()
        s.onTranscript = { got += it }
        val setup = JSONObject(s.setupMessage()).getJSONObject("setup")
        assertNotNull(setup.optJSONObject("inputAudioTranscription"))
        assertNotNull(setup.optJSONObject("outputAudioTranscription"))
        s.connecting()
        s.handle("""{"setupComplete":{}}""", PlaybackState.DRAINED, 0.0)
        s.handle("""{"serverContent":{"inputTranscription":{"text":"Bir hikaye"}}}""", PlaybackState.DRAINED, 0.1)
        s.handle("""{"serverContent":{"inputTranscription":{"text":" anlat"}}}""", PlaybackState.DRAINED, 0.2)
        assertEquals(TranscriptUpdate(TranscriptRole.USER, "Bir hikaye anlat", false, "u1"), got.last())
        val turn = JSONObject().put(
            "serverContent",
            JSONObject().put(
                "modelTurn",
                JSONObject().put(
                    "parts",
                    JSONArray().put(JSONObject().put("inlineData", JSONObject().put("data", pcm(72000)).put("mimeType", "audio/pcm;rate=24000"))),
                ),
            ),
        )
        s.handle(turn.toString(), PlaybackState.QUEUED, 1.0)
        s.handle(JSONObject().put("serverContent", JSONObject().put("outputTranscription", JSONObject().put("text", line))).toString(), PlaybackState.QUEUED, 1.0)
        assertEquals("a 12+ character reply ends the user's turn", 1, got.count { it.role == TranscriptRole.USER && it.final })
        var t = 1.0
        while (t <= 2.5) {
            s.tick(PlaybackState.AUDIBLE, 0.5, t)
            t += 1.0 / 30
        }
        val mid = got.last { it.role == TranscriptRole.ASSISTANT }
        assertTrue(mid.text, line.startsWith(mid.text) && kotlin.math.abs(mid.text.length - line.length / 2) <= 2)
        s.handle("""{"serverContent":{"interrupted":true}}""", PlaybackState.AUDIBLE, 2.55)
        assertTrue(got.last().final && got.last().truncated)
        assertTrue(line.startsWith(got.last().text) && got.last().text.length < line.length)
    }

    @Test fun realtimeTurnsInputTranscriptionOnOnceAndCutsOnABargeIn() {
        val s = OpenAIRealtimeSession(transcribeUser = "gpt-4o-mini-transcribe")
        val sent = mutableListOf<String>()
        s.onSend = { sent += it }
        val got = mutableListOf<TranscriptUpdate>()
        s.onTranscript = { got += it }
        s.connecting()
        s.handle("""{"type":"session.created","session":{"audio":{"input":{"transcription":null}}}}""", 0.0)
        assertEquals(1, sent.size)
        assertEquals(
            "gpt-4o-mini-transcribe",
            JSONObject(sent[0]).getJSONObject("session").getJSONObject("audio").getJSONObject("input").getJSONObject("transcription").getString("model"),
        )
        assertEquals("realtime", JSONObject(sent[0]).getJSONObject("session").getString("type"))
        s.handle("""{"type":"conversation.item.input_audio_transcription.delta","delta":"Bir hikaye"}""", 0.1)
        s.handle("""{"type":"conversation.item.input_audio_transcription.completed","transcript":"Bir hikaye anlat."}""", 0.5)
        assertEquals(TranscriptUpdate(TranscriptRole.USER, "Bir hikaye anlat.", true, "u1"), got.last())
        s.handle("""{"type":"response.created"}""", 1.0)
        s.handle(JSONObject().put("type", "response.output_audio_transcript.delta").put("delta", line).toString(), 1.0)
        s.handle("""{"type":"output_audio_buffer.started"}""", 1.0)
        var t = 1.0
        while (t <= 1.5) {
            s.tick(0.5, t)
            t += 1.0 / 30
        }
        s.handle("""{"type":"input_audio_buffer.speech_started"}""", 1.55)
        val cut = got.last()
        assertTrue(cut.final && cut.truncated)
        assertTrue(cut.text, line.startsWith(cut.text) && cut.text.isNotEmpty() && cut.text.length < line.length)
        s.connecting()
        s.handle("""{"type":"session.created","session":{"audio":{"input":{"transcription":{"model":"whisper-1"}}}}}""")
        assertEquals("a session with its own transcription is left alone", 1, sent.size)
        // Off by default: OpenAI bills it, so only an opt-in turns it on.
        val off = OpenAIRealtimeSession()
        var offSent = 0
        off.onSend = { offSent++ }
        off.onTranscript = {}
        off.connecting()
        off.handle("""{"type":"session.created","session":{}}""")
        assertEquals(0, offSent)
    }

    @Test fun liveKitPassesSegmentsThroughAndEndsTheAgentTurnWhenItStops() {
        val t = LiveKitAgentTracker()
        val got = mutableListOf<TranscriptUpdate>()
        t.onTranscript = { got += it }
        t.start()
        t.participantSeen("agent", true, mapOf("lk.agent.state" to "listening"))
        t.transcription("me", false, emptyMap(), true, "SG_1", "Hava nasıl?", true, 0.0)
        assertEquals(TranscriptUpdate(TranscriptRole.USER, "Hava nasıl?", true, "u1"), got.last())
        t.transcription("guest", false, emptyMap(), false, "SG_9", "Selam", true, 1.0)
        assertEquals("a non-agent participant is not the assistant", 1, got.size)
        t.attributesChanged("agent", true, mapOf("lk.agent.state" to "speaking"), mapOf("lk.agent.state" to "speaking"), false)
        t.transcription("agent", true, emptyMap(), false, "SG_2", "Güneşli ve ılık.", false, 2.0)
        assertEquals(TranscriptUpdate(TranscriptRole.ASSISTANT, "Güneşli ve ılık.", false, "a1"), got.last())
        t.attributesChanged("agent", true, mapOf("lk.agent.state" to "listening"), mapOf("lk.agent.state" to "listening"), false)
        assertEquals(TranscriptUpdate(TranscriptRole.ASSISTANT, "Güneşli ve ılık.", true, "a1"), got.last())
    }
}
