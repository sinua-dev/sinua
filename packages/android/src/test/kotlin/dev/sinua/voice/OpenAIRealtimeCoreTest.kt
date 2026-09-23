package dev.sinua.voice

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The SDK-free core of the native OpenAI Realtime source -- the same cases as the
 * iOS OpenAIRealtimeCoreTests: session rules, reconnect policy, transcript replay,
 * signaling request shape and status mapping.
 */
class OpenAIRealtimeCoreTest {
    private class Box {
        val states = mutableListOf<AgentState>()
        var interrupts = 0
    }

    private fun make(): Pair<OpenAIRealtimeSession, Box> {
        val s = OpenAIRealtimeSession()
        val box = Box()
        s.onState = { box.states += it }
        s.onInterrupt = { box.interrupts++ }
        s.connecting()
        s.connected()
        return s to box
    }

    private fun ev(type: String, extra: String = "") = """{"type":"$type"$extra}"""

    @Test
    fun vadAndResponseLifecycleWithOutputBufferEvents() {
        val (s, box) = make()
        s.handle(ev("input_audio_buffer.speech_started"))
        s.handle(ev("input_audio_buffer.speech_stopped"))
        s.handle(ev("response.created"))
        s.handle(ev("output_audio_buffer.started"))
        s.tick(0.0)
        s.handle(ev("response.done"))
        assertEquals(AgentState.SPEAKING, s.state)
        s.handle(ev("output_audio_buffer.stopped"))
        assertEquals(
            listOf(
                AgentState.INITIALIZING,
                AgentState.LISTENING,
                AgentState.THINKING,
                AgentState.SPEAKING,
                AgentState.LISTENING,
            ),
            box.states,
        )
        assertEquals(0, box.interrupts)
    }

    @Test
    fun bargeInOnlyOverAudibleOutput() {
        val (s, box) = make()
        s.handle(ev("response.created"))
        s.handle(ev("input_audio_buffer.speech_started"))
        assertEquals(0, box.interrupts)
        s.handle(ev("response.created"))
        s.handle(ev("output_audio_buffer.started"))
        s.handle(ev("input_audio_buffer.speech_started"))
        assertEquals(1, box.interrupts)
        s.handle(ev("output_audio_buffer.cleared"))
        assertEquals(AgentState.LISTENING, s.state)
    }

    @Test
    fun responseWithoutAudioDoesNotHang() {
        val (s, _) = make()
        s.handle(ev("response.created"))
        s.handle(ev("response.done"))
        assertEquals(AgentState.LISTENING, s.state)
    }

    @Test
    fun energyFallbackWithoutOutputBufferEvents() {
        val (s, _) = make()
        s.handle(ev("response.created"))
        s.tick(0.2)
        assertEquals(AgentState.SPEAKING, s.state)
        s.handle(ev("response.done"))
        repeat(8) { s.tick(0.01) }
        assertEquals(AgentState.SPEAKING, s.state)
        s.tick(0.01)
        assertEquals(AgentState.LISTENING, s.state)
        s.tick(0.3)
        assertEquals(AgentState.LISTENING, s.state)
    }

    @Test
    fun transcriptAndFatalErrors() {
        val (s, _) = make()
        s.handle(ev("response.output_audio_transcript.done", ""","transcript":" Hello there """"))
        s.handle(ev("conversation.item.input_audio_transcription.completed", ""","transcript":"hi""""))
        s.handle(ev("response.output_audio_transcript.done", ""","transcript":"   """"))
        assertEquals(
            listOf(
                TranscriptLog.Turn(TranscriptLog.Role.ASSISTANT, "Hello there"),
                TranscriptLog.Turn(TranscriptLog.Role.USER, "hi"),
            ),
            s.transcript.window(),
        )
        s.handle(ev("error", ""","error":{"code":"rate_limit_exceeded"}"""))
        assertNull(s.fatalCode)
        s.handle(ev("error", ""","error":{"code":"invalid_api_key"}"""))
        assertEquals("invalid_api_key", s.fatalCode)
        s.connecting()
        assertNull(s.fatalCode)
        s.handle("nope")
        s.stopped()
        assertEquals(AgentState.IDLE, s.state)
    }

    @Test
    fun backoffDelaysLikeWeb() {
        assertEquals(100, RealtimeReconnect.delayMs(1, 0.5))
        assertEquals(1000, RealtimeReconnect.delayMs(2, 0.5))
        assertEquals(2000, RealtimeReconnect.delayMs(3, 0.5))
        assertEquals(8000, RealtimeReconnect.delayMs(9, 0.5))
        assertEquals(800, RealtimeReconnect.delayMs(2, 0.0))
        assertEquals(1200, RealtimeReconnect.delayMs(2, 0.999999))
        assertTrue(RealtimeReconnect.isRetryable(429))
        assertTrue(RealtimeReconnect.isRetryable(503))
        assertFalse(RealtimeReconnect.isRetryable(401))
        assertTrue(RealtimeReconnect.isFatalError("insufficient_quota"))
        assertFalse(RealtimeReconnect.isFatalError(null))
    }

    @Test
    fun transcriptReplayWindowAndEvents() {
        val log = TranscriptLog(maxChars = 12, maxItems = 2)
        log.add(TranscriptLog.Role.USER, "aaaa")
        log.add(TranscriptLog.Role.ASSISTANT, "bbbbbbbb")
        log.add(TranscriptLog.Role.USER, "cccc")
        assertEquals(listOf("bbbbbbbb", "cccc"), log.window().map { it.text })
        val first = JSONObject(log.replayEvents()[0])
        assertEquals("conversation.item.create", first.getString("type"))
        assertEquals("assistant", first.getJSONObject("item").getString("role"))
        assertEquals(
            "output_text",
            first.getJSONObject("item").getJSONArray("content").getJSONObject(0).getString("type"),
        )
        assertEquals(
            "input_text",
            JSONObject(
                log.replayEvents()[1],
            ).getJSONObject("item").getJSONArray("content").getJSONObject(0).getString("type"),
        )
    }

    @Test
    fun signalingRequestShapeAndStatusMapping() {
        val r = OpenAIRealtimeSignaling.callsRequest("v=0\r\noffer", "ek_123")
        assertEquals(OpenAIRealtimeSignaling.CALLS_URL, r.url)
        assertEquals(mapOf("Authorization" to "Bearer ek_123"), r.headers)
        assertEquals("application/sdp", r.contentType)
        assertEquals("v=0\r\noffer", r.body)
        assertEquals("v=0\r\nx", OpenAIRealtimeSignaling.answer(201, "v=0\r\nx"))
        assertTrue(
            runCatching {
                OpenAIRealtimeSignaling.answer(401, "no")
            }.exceptionOrNull() is OpenAIRealtimeSignaling.SignalingException.Fatal,
        )
        assertTrue(
            runCatching {
                OpenAIRealtimeSignaling.answer(503, "")
            }.exceptionOrNull() is OpenAIRealtimeSignaling.SignalingException.Retryable,
        )
        assertTrue(
            runCatching {
                OpenAIRealtimeSignaling.answer(200, "{}")
            }.exceptionOrNull() is OpenAIRealtimeSignaling.SignalingException.Malformed,
        )

        val m = OpenAIRealtimeSignaling.clientSecretRequest("sk-dev", instructions = "Be brief.")
        assertEquals(mapOf("Authorization" to "Bearer sk-dev"), m.headers)
        val session = JSONObject(m.body).getJSONObject("session")
        assertEquals("gpt-realtime", session.getString("model"))
        assertEquals("Be brief.", session.getString("instructions"))
        assertEquals(
            "server_vad",
            session.getJSONObject("audio").getJSONObject("input").getJSONObject("turn_detection").getString("type"),
        )
        assertEquals(600, JSONObject(m.body).getJSONObject("expires_after").getInt("seconds"))
        assertEquals("ek_abc", OpenAIRealtimeSignaling.clientSecret(200, """{"value":"ek_abc"}"""))
        assertTrue(runCatching { OpenAIRealtimeSignaling.clientSecret(200, "{}") }.isFailure)
    }
}
