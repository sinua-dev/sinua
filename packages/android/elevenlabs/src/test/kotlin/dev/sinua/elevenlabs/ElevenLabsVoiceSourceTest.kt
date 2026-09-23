package dev.sinua.elevenlabs

import dev.sinua.voice.AgentState
import dev.sinua.voice.MainDispatcher
import dev.sinua.voice.Pcm
import dev.sinua.voice.PcmAudioDevice
import dev.sinua.voice.VoiceMetrics
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.json.JSONObject
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test
import java.util.Collections
import java.util.concurrent.Callable
import java.util.concurrent.ConcurrentHashMap
import java.util.concurrent.Executors
import java.util.concurrent.ScheduledFuture
import java.util.concurrent.TimeUnit
import kotlin.math.PI
import kotlin.math.sin

/**
 * The real `ElevenLabsVoiceSource` + OkHttp against an in-process fake ElevenLabs
 * server (okhttp3 mockwebserver WebSocket upgrade), with a fake audio device (no
 * mic, no sound) and a single-thread executor as "main". Wire, not vendor.
 */
class ElevenLabsVoiceSourceTest {
    private class ExecutorMain : MainDispatcher {
        val exec = Executors.newSingleThreadScheduledExecutor()
        private val delayed = ConcurrentHashMap<Runnable, ScheduledFuture<*>>()
        override fun post(r: Runnable) {
            exec.execute(r)
        }
        override fun postDelayed(r: Runnable, delayMs: Long) {
            delayed[r] = exec.schedule(r, delayMs, TimeUnit.MILLISECONDS)
        }
        override fun remove(r: Runnable) {
            delayed.remove(r)?.cancel(false)
        }
        fun <T> sync(block: () -> T): T = exec.submit(Callable { block() }).get(5, TimeUnit.SECONDS)
    }

    private class FakeDevice : PcmAudioDevice {
        @Volatile override var playedFrames = 0L
        val scheduled: MutableList<Long> = Collections.synchronizedList(mutableListOf())

        @Volatile var resets = 0

        @Volatile var started = false

        @Volatile var rates: Pair<Int, Int>? = null

        @Volatile var capture: ((FloatArray) -> Unit)? = null
        override var onClockReset: (() -> Unit)? = null
        override fun start(inputRate: Int, outputRate: Int, onCapture: (FloatArray) -> Unit) {
            started = true
            rates = inputRate to outputRate
            capture = onCapture
        }
        override fun schedule(samples: FloatArray, frame: Long) {
            scheduled += frame
        }
        override fun resetPlayback(fade: Boolean) {
            resets++
            playedFrames = 0
        }
        override fun stop() {
            started = false
        }
    }

    private class ServerConn(val onText: (WebSocket, String) -> Unit) : WebSocketListener() {
        val received: MutableList<String> = Collections.synchronizedList(mutableListOf())

        @Volatile var ws: WebSocket? = null
        override fun onOpen(webSocket: WebSocket, response: Response) {
            ws = webSocket
        }
        override fun onMessage(webSocket: WebSocket, text: String) {
            received += text
            onText(webSocket, text)
        }
        override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
            webSocket.close(1000, null)
        }
    }

    private lateinit var server: MockWebServer
    private val main = ExecutorMain()
    private val metadata = """{"type":"conversation_initiation_metadata","conversation_initiation_metadata_event":{"conversation_id":"c1","agent_output_audio_format":"pcm_24000","user_input_audio_format":"pcm_16000"}}"""

    @Before
    fun setUp() {
        server = MockWebServer()
        server.start()
    }

    @After
    fun tearDown() {
        server.shutdown()
        main.exec.shutdownNow()
    }

    /**
     * Polls [cond], then waits for the "main" executor to finish whatever it is running.
     * The source emits a state from inside a main-thread task that goes on to do the
     * work the state implies (start the device, schedule the audio), and this thread
     * polls from outside that task, so it can see the state first. The executor is
     * single-threaded, so an empty task submitted after [cond] turns true runs only
     * once that task is done. Without it the vendor tests failed CI on a slow runner
     * (2026-09-21: `device.started`, then `device.scheduled.first()` on an empty list).
     */
    private fun until(timeoutMs: Long = 5000, cond: () -> Boolean) {
        val end = System.currentTimeMillis() + timeoutMs
        while (!cond()) {
            check(System.currentTimeMillis() < end) { "timed out" }
            Thread.sleep(10)
        }
        main.sync { }
    }

    private fun wsUrl(path: String = "/v1/convai/conversation?agent_id=agent_abc") = server.url(path).toString().replaceFirst("http", "ws")

    private fun audio(id: Int): String {
        val tone = FloatArray(4800) { (0.5 * sin(2 * PI * 440 * it / 24000)).toFloat() }
        return """{"type":"audio","audio_event":{"audio_base_64":"${Pcm.base64Encode(
            Pcm.floatToPcm16(tone),
        )}","event_id":$id}}"""
    }

    @Test
    fun handshakeFormatsTurnPingInterruptionAndAgentEnd() {
        val conn =
            ServerConn { ws, text -> if (text.contains("conversation_initiation_client_data")) ws.send(metadata) }
        server.enqueue(MockResponse().withWebSocketUpgrade(conn))
        val device = FakeDevice()
        val source = ElevenLabsVoiceSource(
            "agent_abc",
            overrides = JSONObject().put("agent", JSONObject().put("language", "tr")),
            endpoint = wsUrl(),
            device = device,
            main = main,
        )
        val states: MutableList<AgentState> = Collections.synchronizedList(mutableListOf())
        val metrics: MutableList<VoiceMetrics> = Collections.synchronizedList(mutableListOf())
        var interrupts = 0
        source.onStateChange { states += it }
        source.onMetrics { metrics += it }
        source.onInterrupt { interrupts++ }
        main.sync { source.connect() }

        val req = server.takeRequest(5, TimeUnit.SECONDS)!!
        assertEquals("convai", req.headers["Sec-WebSocket-Protocol"])
        assertTrue(req.path!!.contains("agent_id=agent_abc"))
        until { states.lastOrNull() == AgentState.LISTENING }
        assertTrue(conn.received.first().contains("\"language\":\"tr\""))
        assertEquals(16000 to 24000, device.rates)
        assertEquals(listOf(AgentState.INITIALIZING, AgentState.LISTENING), states.toList())

        device.capture!!.invoke(FloatArray(1024) { 0.25f })
        until { conn.received.any { it.contains("user_audio_chunk") } }

        conn.ws!!.send("""{"type":"ping","ping_event":{"event_id":42,"ping_ms":30}}""")
        until { conn.received.any { it.contains("\"pong\"") && it.contains("\"event_id\":42") } }

        conn.ws!!.send(audio(1))
        until { states.lastOrNull() == AgentState.THINKING }
        assertEquals(2400L, device.scheduled.first())
        device.playedFrames = 2400L + 2400
        until { states.lastOrNull() == AgentState.SPEAKING }
        until { (metrics.lastOrNull()?.level ?: 0.0) > 0.05 }
        // The server's agent_response_complete + drained ends the agent's turn.
        conn.ws!!.send("""{"type":"agent_response_complete","agent_response_complete_event":{"event_id":1}}""")
        device.playedFrames = 2400L + 4800
        until { states.lastOrNull() == AgentState.LISTENING }

        conn.ws!!.send(audio(2))
        until { device.scheduled.size == 2 }
        device.playedFrames = device.scheduled.last() + 100
        until { states.lastOrNull() == AgentState.SPEAKING }
        conn.ws!!.send("""{"type":"interruption","interruption_event":{"event_id":3}}""")
        until { interrupts == 1 && device.resets == 1 }
        conn.ws!!.send(audio(2)) // late chunk of the cut-off response
        conn.ws!!.send("""{"type":"ping","ping_event":{"event_id":43,"ping_ms":30}}""")
        until { conn.received.any { it.contains("\"event_id\":43") } } // everything before it was handled
        assertEquals("the late chunk was dropped", 2, device.scheduled.size)

        conn.ws!!.close(1000, "agent ended")
        until { states.lastOrNull() == AgentState.IDLE }
        assertFalse(device.started)
        assertEquals("no reconnect", 1, server.requestCount)
    }

    @Test
    fun nonPcmInputAndBadAgentFailWithoutTheMic() {
        server.enqueue(
            MockResponse().withWebSocketUpgrade(
                ServerConn { ws, _ ->
                    ws.send(
                        """{"type":"conversation_initiation_metadata","conversation_initiation_metadata_event":{"agent_output_audio_format":"pcm_16000","user_input_audio_format":"ulaw_8000"}}""",
                    )
                },
            ),
        )
        server.enqueue(MockResponse().withWebSocketUpgrade(ServerConn { ws, _ -> ws.close(1008, "unknown agent") }))
        for (expect in listOf("ulaw_8000", "closed")) {
            val device = FakeDevice()
            val source = ElevenLabsVoiceSource("a", endpoint = wsUrl("/"), device = device, main = main)
            val errors: MutableList<Throwable> = Collections.synchronizedList(mutableListOf())
            val states: MutableList<AgentState> = Collections.synchronizedList(mutableListOf())
            source.onError { errors += it }
            source.onStateChange { states += it }
            main.sync { source.connect() }
            until { errors.isNotEmpty() }
            assertTrue("$expect: ${errors.single().message}", errors.single().message!!.contains(expect))
            assertEquals(AgentState.IDLE, states.last())
            assertNull("the audio never started", device.rates)
        }
    }

    @Test
    fun missingCredentialThrows() {
        val err = runCatching {
            main.sync { ElevenLabsVoiceSource(" ", device = FakeDevice(), main = main).connect() }
        }.exceptionOrNull()
        assertTrue(err?.cause is IllegalStateException)
    }
}
