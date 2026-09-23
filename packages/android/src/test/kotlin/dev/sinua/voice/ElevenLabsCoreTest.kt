package dev.sinua.voice

import org.json.JSONObject
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The SDK-free core of the native ElevenLabs source -- the same cases as the iOS
 * ElevenLabsCoreTests: `Pcm` μ-law / format parsing (pcm.ts) and
 * `ElevenLabsSession` (ElevenLabsVoiceSource.ts's protocol + state machine).
 */
class ElevenLabsCoreTest {
    private class Box {
        val states = mutableListOf<AgentState>()
        var interrupts = 0
    }

    private fun make(): Pair<ElevenLabsSession, Box> {
        val s = ElevenLabsSession()
        val box = Box()
        s.onState = { box.states += it }
        s.onInterrupt = { box.interrupts++ }
        return s to box
    }

    private val pcm44 = Pcm.AudioFormat(Pcm.AudioFormat.Codec.PCM, 44100)
    private val metadata = """{"type":"conversation_initiation_metadata","conversation_initiation_metadata_event":{"conversation_id":"c1","agent_output_audio_format":"pcm_44100","user_input_audio_format":"pcm_16000"}}"""

    private fun audio(id: Int, n: Int = 160) = """{"type":"audio","audio_event":{"audio_base_64":"${Pcm.base64Encode(
        Pcm.floatToPcm16(
            FloatArray(n) {
                0.25f
            },
        ),
    )}","event_id":$id}}"""

    private fun audio(id: Int, isFinal: Boolean) = """{"type":"audio","audio_event":{"audio_base_64":"${Pcm.base64Encode(
        Pcm.floatToPcm16(
            FloatArray(160) {
                0.25f
            },
        ),
    )}","event_id":$id,"is_final":$isFinal}}"""

    private val complete = """{"type":"agent_response_complete","agent_response_complete_event":{"event_id":9}}"""
    private val agentText = """{"type":"agent_response","agent_response_event":{"agent_response":"Hello.","event_id":9}}"""
    private val transcript = """{"type":"user_transcript","user_transcription_event":{"user_transcript":"hi"}}"""

    private fun started(s: ElevenLabsSession) {
        s.connecting(0.0)
        s.handle(metadata, PlaybackState.DRAINED, 0.0)
        s.graphStarted(pcm44, 0.0)
    }

    @Test
    fun parseAudioFormatLikeWeb() {
        assertEquals(pcm44, Pcm.parseAudioFormat("pcm_44100"))
        assertEquals(Pcm.AudioFormat(Pcm.AudioFormat.Codec.ULAW, 8000), Pcm.parseAudioFormat(" ULAW_8000 "))
        val fallback = Pcm.AudioFormat(Pcm.AudioFormat.Codec.PCM, 16000)
        assertEquals(fallback, Pcm.parseAudioFormat("mp3_44100"))
        assertEquals(fallback, Pcm.parseAudioFormat(null))
        assertEquals(fallback, Pcm.parseAudioFormat("pcm_16000_x"))
    }

    @Test
    fun ulawDecodeMatchesTheG711Table() {
        val out = Pcm.ulawToFloat(byteArrayOf(0xFF.toByte(), 0x7F, 0x80.toByte(), 0x00, 0xF0.toByte()))
        assertArrayEquals(
            floatArrayOf(0f, 0f, (16764 + (15 shl 10)) / 32768f, -(16764 + (15 shl 10)) / 32768f, (15 shl 3) / 32768f),
            out,
            0f,
        )
    }

    @Test
    fun endpointInitAndMicMessages() {
        assertEquals(
            "wss://api.elevenlabs.io/v1/convai/conversation?agent_id=agent_abc",
            ElevenLabsSession.endpoint(" agent_abc "),
        )
        assertEquals("wss://signed.example/x?token=1", ElevenLabsSession.endpoint("wss://signed.example/x?token=1"))
        assertEquals(
            "conversation_initiation_client_data",
            JSONObject(ElevenLabsSession.initMessage()).getString("type"),
        )
        val ov =
            JSONObject(ElevenLabsSession.initMessage(JSONObject().put("agent", JSONObject().put("language", "tr"))))
        assertEquals(
            "tr",
            ov.getJSONObject("conversation_config_override").getJSONObject("agent").getString("language"),
        )
        assertEquals(
            Pcm.base64Encode(Pcm.floatToPcm16(floatArrayOf(0.5f, -0.5f))),
            JSONObject(ElevenLabsSession.micMessage(floatArrayOf(0.5f, -0.5f))).getString("user_audio_chunk"),
        )
    }

    @Test
    fun metadataAndPingPong() {
        val (s, _) = make()
        s.connecting(0.0)
        assertEquals(
            listOf(ElevenLabsSession.Action.Metadata(Pcm.AudioFormat(Pcm.AudioFormat.Codec.PCM, 16000), pcm44)),
            s.handle(metadata, PlaybackState.DRAINED, 0.0),
        )
        val pong = s.handle(
            """{"type":"ping","ping_event":{"event_id":7,"ping_ms":40}}""",
            PlaybackState.DRAINED,
            0.0,
        ).single()
            as ElevenLabsSession.Action.Send
        assertEquals("pong", JSONObject(pong.text).getString("type"))
        assertEquals(7, JSONObject(pong.text).getInt("event_id"))
    }

    @Test
    fun pendingAudioIsFlushedWhenTheGraphStarts() {
        val (s, box) = make()
        s.connecting(0.0)
        s.handle(metadata, PlaybackState.DRAINED, 0.0)
        assertTrue(s.handle(audio(1), PlaybackState.DRAINED, 0.0).isEmpty())
        val flushed = s.graphStarted(pcm44, 0.0).single() as ElevenLabsSession.Action.Enqueue
        assertEquals(160, flushed.samples.size)
        assertEquals(44100, flushed.rate)
        assertEquals(listOf(AgentState.INITIALIZING, AgentState.THINKING), box.states)
    }

    @Test
    fun turnFollowsVadTranscriptAndPlayback() {
        val (s, box) = make()
        started(s)
        s.handle("""{"type":"vad_score","vad_score_event":{"vad_score":0.9}}""", PlaybackState.DRAINED, 1.0)
        s.handle("""{"type":"vad_score","vad_score_event":{"vad_score":0.1}}""", PlaybackState.DRAINED, 2.0)
        assertEquals(AgentState.THINKING, s.state)
        s.handle(audio(2), PlaybackState.DRAINED, 2.5)
        s.tick(PlaybackState.QUEUED, 2.6)
        assertEquals(AgentState.THINKING, s.state)
        s.tick(PlaybackState.AUDIBLE, 2.7)
        assertEquals(AgentState.SPEAKING, s.state)
        s.handle(complete, PlaybackState.AUDIBLE, 2.8)
        assertEquals(AgentState.SPEAKING, s.state)
        s.tick(PlaybackState.DRAINED, 3.0)
        assertEquals(AgentState.LISTENING, s.state)
        s.handle(
            """{"type":"user_transcript","user_transcription_event":{"user_transcript":"hi"}}""",
            PlaybackState.DRAINED,
            4.0,
        )
        assertEquals(AgentState.THINKING, s.state)
        s.tick(PlaybackState.DRAINED, 7.9)
        assertEquals(AgentState.THINKING, s.state)
        s.tick(PlaybackState.DRAINED, 8.1)
        assertEquals(AgentState.LISTENING, s.state)
        assertEquals(0, box.interrupts)
    }

    @Test
    fun vadWhileTheAgentSpeaksDoesNotStealTheState() {
        val (s, _) = make()
        started(s)
        s.handle(audio(1), PlaybackState.DRAINED, 0.0)
        s.tick(PlaybackState.AUDIBLE, 0.2)
        s.handle("""{"type":"vad_score","vad_score_event":{"vad_score":0.95}}""", PlaybackState.AUDIBLE, 0.3)
        assertEquals(AgentState.SPEAKING, s.state)
    }

    @Test
    fun interruptionDropsLateChunksAndFlashesOnlyWithOutput() {
        val (s, box) = make()
        started(s)
        s.handle(audio(3), PlaybackState.DRAINED, 0.0)
        s.tick(PlaybackState.AUDIBLE, 0.1)
        assertEquals(
            listOf(ElevenLabsSession.Action.ClearPlayback(true)),
            s.handle("""{"type":"interruption","interruption_event":{"event_id":5}}""", PlaybackState.AUDIBLE, 0.2),
        )
        assertEquals(1, box.interrupts)
        assertEquals(AgentState.LISTENING, s.state)
        assertTrue(s.handle(audio(4), PlaybackState.DRAINED, 0.3).isEmpty())
        assertEquals(1, s.handle(audio(5), PlaybackState.DRAINED, 0.3).size)
        s.handle("""{"type":"interruption","interruption_event":{"event_id":9}}""", PlaybackState.DRAINED, 1.0)
        assertEquals(1, box.interrupts)
    }

    @Test
    fun ulawOutputIsDecodedAtItsRate() {
        val (s, _) = make()
        s.connecting(0.0)
        s.graphStarted(Pcm.AudioFormat(Pcm.AudioFormat.Codec.ULAW, 8000), 0.0)
        val bytes = byteArrayOf(0x80.toByte(), 0x00)
        val e = s.handle(
            """{"type":"audio","audio_event":{"audio_base_64":"${Pcm.base64Encode(bytes)}","event_id":1}}""",
            PlaybackState.DRAINED,
            0.0,
        )
            .single() as ElevenLabsSession.Action.Enqueue
        assertEquals(8000, e.rate)
        assertArrayEquals(Pcm.ulawToFloat(bytes), e.samples, 0f)
    }

    @Test
    fun malformedAndIrrelevantFrames() {
        val (s, box) = make()
        started(s)
        assertTrue(s.handle("nope", PlaybackState.DRAINED, 0.0).isEmpty())
        assertTrue(s.handle("""{"type":"agent_response_complete"}""", PlaybackState.DRAINED, 0.0).isEmpty())
        assertEquals(AgentState.LISTENING, box.states.last())
        s.stopped(1.0)
        assertEquals(AgentState.IDLE, s.state)
    }

    // Turn end: agent_response_complete / is_final -- the same cases as the iOS ElevenLabsCoreTests.

    @Test
    fun drainedMidReplyIsAStallNotTheEndOfTheTurn() {
        val (s, _) = make()
        started(s)
        s.handle(audio(1), PlaybackState.DRAINED, 1.0)
        s.tick(PlaybackState.AUDIBLE, 1.2)
        assertEquals(AgentState.SPEAKING, s.state)
        s.tick(PlaybackState.DRAINED, 1.5)
        assertEquals(AgentState.THINKING, s.state)
        s.handle(audio(1), PlaybackState.DRAINED, 1.7)
        s.tick(PlaybackState.AUDIBLE, 1.9)
        assertEquals(AgentState.SPEAKING, s.state)
        s.handle(complete, PlaybackState.AUDIBLE, 2.0)
        s.tick(PlaybackState.DRAINED, 2.3)
        assertEquals(AgentState.LISTENING, s.state)
    }

    @Test
    fun aFinalChunkEndsTheReplyLikeComplete() {
        val (s, _) = make()
        started(s)
        s.handle(audio(1, false), PlaybackState.DRAINED, 1.0)
        s.handle(audio(1, true), PlaybackState.QUEUED, 1.1)
        s.tick(PlaybackState.AUDIBLE, 1.3)
        s.tick(PlaybackState.DRAINED, 1.8)
        assertEquals(AgentState.LISTENING, s.state)
    }

    @Test
    fun aReplyWithoutAudioEndsThinkingAtOnce() {
        val (s, _) = make()
        started(s)
        s.handle(transcript, PlaybackState.DRAINED, 1.0)
        s.handle(agentText, PlaybackState.DRAINED, 1.5)
        assertEquals(AgentState.THINKING, s.state)
        s.handle(complete, PlaybackState.DRAINED, 1.6)
        assertEquals(AgentState.LISTENING, s.state)
    }

    @Test
    fun aStalledReplyGivesUpAfterTenSeconds() {
        val (s, _) = make()
        started(s)
        s.handle(audio(1), PlaybackState.DRAINED, 1.0)
        s.tick(PlaybackState.AUDIBLE, 1.2)
        s.tick(PlaybackState.DRAINED, 1.5)
        assertEquals(AgentState.THINKING, s.state)
        s.tick(PlaybackState.DRAINED, 6.0)
        assertEquals(AgentState.THINKING, s.state)
        s.tick(PlaybackState.DRAINED, 11.1)
        assertEquals(AgentState.LISTENING, s.state)
    }

    @Test
    fun chunksAfterCompleteDoNotReopenTheReply() {
        val (s, _) = make()
        started(s)
        s.handle(audio(1), PlaybackState.DRAINED, 1.0)
        s.handle(complete, PlaybackState.QUEUED, 1.05)
        s.handle(audio(1), PlaybackState.QUEUED, 1.1)
        s.tick(PlaybackState.AUDIBLE, 1.3)
        s.tick(PlaybackState.DRAINED, 1.9)
        assertEquals(AgentState.LISTENING, s.state)
        s.handle(transcript, PlaybackState.DRAINED, 2.0)
        s.handle(audio(2), PlaybackState.DRAINED, 2.5)
        s.tick(PlaybackState.AUDIBLE, 2.7)
        s.tick(PlaybackState.DRAINED, 3.0)
        assertEquals(AgentState.THINKING, s.state)
    }

    @Test
    fun interruptionClosesTheReply() {
        val (s, box) = make()
        started(s)
        s.handle(audio(1), PlaybackState.DRAINED, 1.0)
        s.tick(PlaybackState.AUDIBLE, 1.2)
        s.handle("""{"type":"interruption","interruption_event":{"event_id":3}}""", PlaybackState.AUDIBLE, 1.4)
        assertEquals(AgentState.LISTENING, s.state)
        assertEquals(1, box.interrupts)
        s.tick(PlaybackState.DRAINED, 1.6)
        assertEquals(AgentState.LISTENING, s.state)
    }
}
