package dev.sinua.voice

import org.json.JSONObject
import org.junit.Assert.assertArrayEquals
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test
import kotlin.math.PI
import kotlin.math.sin

/** A `PcmAudioDevice` whose clock the test drives. */
class FakePcmDevice : PcmAudioDevice {
    override var playedFrames = 0L
    val scheduled = mutableListOf<Pair<Long, Int>>()
    var resets = 0
    var started = false
    var capture: ((FloatArray) -> Unit)? = null
    override var onClockReset: (() -> Unit)? = null

    override fun start(inputRate: Int, outputRate: Int, onCapture: (FloatArray) -> Unit) {
        started = true
        capture = onCapture
    }

    override fun schedule(samples: FloatArray, frame: Long) {
        scheduled += frame to samples.size
    }

    override fun resetPlayback(fade: Boolean) {
        resets++
        playedFrames = 0
        scheduled.clear()
    }

    override fun stop() {
        started = false
    }
}

/**
 * The SDK-free core of the native Gemini Live source -- the same cases as the iOS
 * GeminiLiveCoreTests: `Pcm` (pcm.ts), the playback timeline (PcmAudioGraph.ts),
 * and `GeminiLiveSession` (PCMStreamVoiceSource.ts's protocol + state machine).
 */
class GeminiLiveCoreTest {
    // --- Pcm

    @Test
    fun pcm16RoundTripClampsAndRoundsLikeWeb() {
        val data = Pcm.floatToPcm16(floatArrayOf(0f, 0.5f, -0.5f, 1f, -1f, 2f, -2f, 1e-5f))
        assertEquals(16, data.size)
        assertArrayEquals(
            floatArrayOf(0f, 0.5f, -0.5f, 32767 / 32768f, -1f, 32767 / 32768f, -1f, 0f),
            Pcm.pcm16ToFloat(data),
            0f,
        )
        assertArrayEquals(byteArrayOf(0x00, 0x40), Pcm.floatToPcm16(floatArrayOf(0.5f)))
        assertArrayEquals(floatArrayOf(0.5f), Pcm.pcm16ToFloat(byteArrayOf(0x00, 0x40, 0xFF.toByte())), 0f)
        // Exact negative half: JS Math.round(-0.5) is -0 -> 0, not -1.
        assertArrayEquals(byteArrayOf(0, 0), Pcm.floatToPcm16(floatArrayOf(-1f / 65536)))
    }

    @Test
    fun base64MatchesTheStandardCodec() {
        for (n in 0..40) {
            val bytes = ByteArray(n) { (it * 37 + 11).toByte() }
            val enc = Pcm.base64Encode(bytes)
            assertEquals(java.util.Base64.getEncoder().encodeToString(bytes), enc)
            assertArrayEquals(bytes, Pcm.base64Decode(enc))
        }
        assertNull(Pcm.base64Decode("ab!c"))
    }

    @Test
    fun parseRateAndResample() {
        assertEquals(24000, Pcm.parseRate("audio/pcm;rate=24000"))
        assertEquals(16000, Pcm.parseRate("audio/pcm; RATE=16000"))
        assertEquals(22050, Pcm.parseRate("audio/pcm", 22050))
        assertEquals(24000, Pcm.parseRate(null))
        val s = floatArrayOf(0f, 1f, 0f, -1f)
        assertTrue(Pcm.resample(s, 24000, 24000) === s)
        assertEquals(8, Pcm.resample(s, 12000, 24000).size)
        assertEquals(0.5f, Pcm.resample(s, 12000, 24000)[1], 1e-6f)
    }

    // --- PlaybackTimeline

    @Test
    fun timelineLeadCursorAndStates() {
        val t = PlaybackTimeline(24000)
        assertEquals(PlaybackState.DRAINED, t.state(0))
        assertEquals(2500L, t.append(FloatArray(1000) { 0.1f }, 100))
        assertEquals(3500L, t.append(FloatArray(1000) { 0.2f }, 200))
        assertEquals(PlaybackState.QUEUED, t.state(2499))
        assertEquals(PlaybackState.AUDIBLE, t.state(2500))
        assertEquals(PlaybackState.AUDIBLE, t.state(4499))
        assertEquals(PlaybackState.DRAINED, t.state(4500))
        assertEquals(7400L, t.append(FloatArray(10) { 0.3f }, 5000))
        assertEquals(PlaybackState.QUEUED, t.state(7000))
        t.clear()
        assertEquals(PlaybackState.DRAINED, t.state(7000))
        assertEquals(0L, t.cursor)
    }

    @Test
    fun timelinePlayedReturnsOnlyWhatHasPlayed() {
        val t = PlaybackTimeline(24000)
        t.append(FloatArray(100) { 0.5f }, 0)
        assertArrayEquals(FloatArray(2400), t.played(0, 2400), 0f)
        assertArrayEquals(FloatArray(10) + FloatArray(10) { 0.5f }, t.played(2390, 2410), 0f)
        assertEquals(512, t.played(0, 3000, 512).size)
    }

    // --- PcmAudioGraph over a fake device

    @Test
    fun graphSchedulesOnTheDeviceClockAndAnalysesPlayedSamplesOnly() {
        val dev = FakePcmDevice()
        val g = PcmAudioGraph(dev)
        assertNull(g.read())
        g.start(16000, 24000) {}
        val tone = FloatArray(4800) { (0.5 * sin(2 * PI * 440 * it / 24000)).toFloat() }
        g.enqueue(tone, 24000)
        assertEquals(2400L, dev.scheduled.first().first)
        assertEquals(PlaybackState.QUEUED, g.playbackState())
        assertEquals(0.0, g.read()!!.level, 0.0)
        dev.playedFrames = 2400L + 2000
        assertEquals(PlaybackState.AUDIBLE, g.playbackState())
        assertTrue(g.read()!!.level > 0.05)
        dev.playedFrames = 2400L + 4800
        assertEquals(PlaybackState.DRAINED, g.playbackState())
        g.clearPlayback(true)
        assertEquals(1, dev.resets)
        g.enqueue(tone, 24000)
        assertEquals(PlaybackState.QUEUED, g.playbackState())
        dev.playedFrames = 0
        dev.onClockReset?.invoke() // the device's clock restarted: queued audio is gone
        assertEquals(PlaybackState.DRAINED, g.playbackState())
        g.stop()
        assertFalse(dev.started)
    }

    // --- GeminiLiveSession

    private class Box {
        val states = mutableListOf<AgentState>()
        var interrupts = 0
    }

    private fun make(): Pair<GeminiLiveSession, Box> {
        val s = GeminiLiveSession("gemini-3.8-live", "Be brief.")
        val box = Box()
        s.onState = { box.states += it }
        s.onInterrupt = { box.interrupts++ }
        return s to box
    }

    private fun audio(n: Int = 480): String {
        val b64 = Pcm.base64Encode(Pcm.floatToPcm16(FloatArray(n) { 0.25f }))
        return """{"serverContent":{"modelTurn":{"parts":[{"inlineData":{"data":"$b64","mimeType":"audio/pcm;rate=24000"}}]}}}"""
    }

    @Test
    fun endpointRoutesCredentialsToHeaders() {
        val token = GeminiLiveSession.endpoint(" auth_tokens/abc ")
        assertEquals(
            "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContentConstrained",
            token.url,
        )
        assertEquals(mapOf("Authorization" to "Token auth_tokens/abc"), token.headers)
        val key = GeminiLiveSession.endpoint("AIzaKEY")
        assertTrue(key.url.endsWith(".BidiGenerateContent"))
        assertEquals(mapOf("x-goog-api-key" to "AIzaKEY"), key.headers)
        assertFalse("no secret in the URL", key.url.contains("?"))
    }

    @Test
    fun setupAndMicMessagesMatchWebShape() {
        val (s, _) = make()
        val setup = JSONObject(s.setupMessage()).getJSONObject("setup")
        assertEquals("models/gemini-3.8-live", setup.getString("model"))
        assertEquals("AUDIO", setup.getJSONObject("generationConfig").getJSONArray("responseModalities").getString(0))
        assertTrue(setup.has("inputAudioTranscription"))
        assertEquals(0, setup.getJSONObject("sessionResumption").length())
        assertTrue(setup.getJSONObject("contextWindowCompression").has("slidingWindow"))
        assertEquals(
            "Be brief.",
            setup.getJSONObject("systemInstruction").getJSONArray("parts").getJSONObject(0).getString("text"),
        )
        assertEquals(
            "models/x",
            JSONObject(GeminiLiveSession("models/x").setupMessage()).getJSONObject("setup").getString("model"),
        )
        val audio = JSONObject(
            GeminiLiveSession.micMessage(floatArrayOf(0.5f, -0.5f)),
        ).getJSONObject("realtimeInput").getJSONObject("audio")
        assertEquals("audio/pcm;rate=16000", audio.getString("mimeType"))
        assertEquals(Pcm.base64Encode(Pcm.floatToPcm16(floatArrayOf(0.5f, -0.5f))), audio.getString("data"))
    }

    @Test
    fun turnFollowsThePlaybackTimeline() {
        val (s, box) = make()
        s.connecting()
        assertEquals(
            listOf(GeminiLiveSession.Action.SetupComplete),
            s.handle("""{"setupComplete":{}}""", PlaybackState.DRAINED),
        )
        s.tick(PlaybackState.DRAINED)
        val enq = s.handle(audio(), PlaybackState.DRAINED).single() as GeminiLiveSession.Action.Enqueue
        assertEquals(480, enq.samples.size)
        assertEquals(24000, enq.rate)
        s.tick(PlaybackState.QUEUED)
        assertEquals(AgentState.THINKING, s.state)
        s.tick(PlaybackState.AUDIBLE)
        assertEquals(AgentState.SPEAKING, s.state)
        s.tick(PlaybackState.DRAINED)
        assertEquals("buffer starved mid-generation", AgentState.THINKING, s.state)
        s.handle(audio(), PlaybackState.DRAINED)
        s.tick(PlaybackState.AUDIBLE)
        s.handle("""{"serverContent":{"turnComplete":true}}""", PlaybackState.AUDIBLE)
        assertEquals(AgentState.SPEAKING, s.state)
        s.tick(PlaybackState.DRAINED)
        assertEquals(AgentState.LISTENING, s.state)
        assertEquals(
            listOf(
                AgentState.INITIALIZING,
                AgentState.LISTENING,
                AgentState.THINKING,
                AgentState.SPEAKING,
                AgentState.THINKING,
                AgentState.SPEAKING,
                AgentState.LISTENING,
            ),
            box.states,
        )
        assertEquals(0, box.interrupts)
    }

    @Test
    fun interruptedCutsPlaybackAndFlashesOnlyIfThereWasOutput() {
        val (s, box) = make()
        s.connecting()
        s.handle("""{"setupComplete":{}}""", PlaybackState.DRAINED)
        s.handle(audio(), PlaybackState.DRAINED)
        s.tick(PlaybackState.AUDIBLE)
        assertEquals(
            listOf(GeminiLiveSession.Action.ClearPlayback(true)),
            s.handle("""{"serverContent":{"interrupted":true}}""", PlaybackState.AUDIBLE),
        )
        assertEquals(1, box.interrupts)
        assertEquals(AgentState.LISTENING, s.state)
        s.handle("""{"serverContent":{"interrupted":true}}""", PlaybackState.DRAINED)
        assertEquals(1, box.interrupts)
        s.handle(audio(), PlaybackState.DRAINED)
        s.handle("""{"serverContent":{"interrupted":true}}""", PlaybackState.QUEUED)
        assertEquals(2, box.interrupts)
    }

    @Test
    fun inputTranscriptionMeansListeningUnlessSpeaking() {
        val (s, _) = make()
        s.connecting()
        s.handle("""{"setupComplete":{}}""", PlaybackState.DRAINED)
        s.handle(audio(), PlaybackState.DRAINED)
        assertEquals(AgentState.THINKING, s.state)
        s.handle("""{"serverContent":{"inputTranscription":{"text":"hi"}}}""", PlaybackState.QUEUED)
        assertEquals(AgentState.LISTENING, s.state)
        s.tick(PlaybackState.AUDIBLE)
        s.handle("""{"serverContent":{"interimInputTranscription":{"text":"h"}}}""", PlaybackState.AUDIBLE)
        assertEquals(AgentState.SPEAKING, s.state)
    }

    @Test
    fun resumptionHandleGoAwayAndReset() {
        val (s, _) = make()
        s.connecting()
        s.handle("""{"setupComplete":{}}""", PlaybackState.DRAINED)
        s.handle("""{"sessionResumptionUpdate":{"newHandle":"h1","resumable":false}}""", PlaybackState.DRAINED)
        assertNull(s.resumptionHandle)
        s.handle("""{"sessionResumptionUpdate":{"newHandle":"h2","resumable":true}}""", PlaybackState.DRAINED)
        assertEquals("h2", s.resumptionHandle)
        assertEquals(
            listOf(GeminiLiveSession.Action.Reconnect("goAway (timeLeft 5s)")),
            s.handle("""{"goAway":{"timeLeft":"5s"}}""", PlaybackState.DRAINED),
        )
        s.connecting()
        assertEquals(AgentState.INITIALIZING, s.state)
        assertEquals(
            "h2",
            JSONObject(s.setupMessage()).getJSONObject("setup").getJSONObject("sessionResumption").getString("handle"),
        )
        s.tick(PlaybackState.AUDIBLE)
        assertEquals(AgentState.INITIALIZING, s.state)
        s.reset()
        assertNull(s.resumptionHandle)
        assertEquals(AgentState.IDLE, s.state)
    }

    @Test
    fun malformedAndErrorFrames() {
        val (s, _) = make()
        assertTrue(s.handle("not json", PlaybackState.DRAINED).isEmpty())
        assertTrue(
            s.handle(
                """{"error":{"code":400}}""",
                PlaybackState.DRAINED,
            ).single() is GeminiLiveSession.Action.ServerError,
        )
        assertTrue(
            s.handle("""{"serverContent":{"modelTurn":{"parts":[{"text":"hi"}]}}}""", PlaybackState.DRAINED).isEmpty(),
        )
    }
}
