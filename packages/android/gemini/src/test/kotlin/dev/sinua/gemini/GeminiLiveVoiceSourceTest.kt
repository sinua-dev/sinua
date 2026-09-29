package dev.sinua.gemini

import dev.sinua.voice.AgentState
import dev.sinua.voice.GeminiLiveSession
import dev.sinua.voice.MainDispatcher
import dev.sinua.voice.Pcm
import dev.sinua.voice.PcmAudioDevice
import dev.sinua.voice.VoiceMetrics
import okhttp3.Response
import okhttp3.WebSocket
import okhttp3.WebSocketListener
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
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
 * The real `GeminiLiveVoiceSource` + OkHttp against an in-process fake Gemini
 * Live server (okhttp3 mockwebserver WebSocket upgrade), with a fake audio device whose
 * clock the test drives and a single-thread executor as "main". Checks the wire,
 * not the vendor: the live API is untested here.
 */
class GeminiLiveVoiceSourceTest {
    /** "Main thread" for the source: one scheduled executor. */
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

        @Volatile var capture: ((FloatArray) -> Unit)? = null

        @Volatile var failStart = false
        override var onClockReset: (() -> Unit)? = null
        override fun start(inputRate: Int, outputRate: Int, onCapture: (FloatArray) -> Unit) {
            if (failStart) throw SecurityException("RECORD_AUDIO not granted")
            started = true
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

    /** Server side of one upgraded connection: records frames, answers `setup` if told to. */
    private class ServerConn(val answerSetup: Boolean) : WebSocketListener() {
        val received: MutableList<String> = Collections.synchronizedList(mutableListOf())

        @Volatile var ws: WebSocket? = null
        override fun onOpen(webSocket: WebSocket, response: Response) {
            ws = webSocket
        }
        override fun onMessage(webSocket: WebSocket, text: String) {
            received += text
            if (text.contains("\"setup\"")) {
                if (answerSetup) webSocket.send("""{"setupComplete":{}}""") else webSocket.close(1008, "rejected")
            }
        }
        override fun onClosing(webSocket: WebSocket, code: Int, reason: String) {
            webSocket.close(1000, null) // complete the close handshake so the server can shut down
        }
    }

    private lateinit var server: MockWebServer
    private val main = ExecutorMain()

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

    private fun wsUrl() = server.url("/live").toString().replaceFirst("http", "ws")

    @Test
    fun mutedSendsZeroedPcmAndReportsTheConnection() {
        val c1 = ServerConn(answerSetup = true)
        server.enqueue(MockResponse().withWebSocketUpgrade(c1))
        val device = FakeDevice()
        val headers = GeminiLiveSession.endpoint("auth_tokens/t1").headers
        val source = GeminiLiveVoiceSource(
            "auth_tokens/t1",
            endpoint = GeminiLiveSession.Endpoint(wsUrl(), headers),
            device = device,
            main = main,
        )
        val conn: MutableList<Boolean> = Collections.synchronizedList(mutableListOf())
        source.onConnectionChange { conn += it }
        assertTrue(source.supportsMute)
        main.sync { source.connect() }
        until { device.started && conn.isNotEmpty() }
        assertEquals(listOf(true), conn.toList())
        source.setMuted(true)
        device.capture!!.invoke(FloatArray(512) { 0.25f })
        until { c1.received.any { it.contains("realtimeInput") } }
        val msg = org.json.JSONObject(c1.received.last { it.contains("realtimeInput") })
        val b64 = msg.getJSONObject("realtimeInput").getJSONObject("audio").getString("data")
        val pcm = java.util.Base64.getDecoder().decode(b64)
        assertEquals("512 samples of PCM16", 1024, pcm.size)
        assertTrue("silence goes out", pcm.all { it == 0.toByte() })
        main.sync { source.disconnect() }
        assertEquals(listOf(true, false), conn.toList())
    }

    @Test
    fun handshakeMicTurnGoAwayResumeAndDisconnect() {
        val c1 = ServerConn(answerSetup = true)
        val c2 = ServerConn(answerSetup = true)
        server.enqueue(MockResponse().withWebSocketUpgrade(c1))
        server.enqueue(MockResponse().withWebSocketUpgrade(c2))
        val device = FakeDevice()
        val headers = GeminiLiveSession.endpoint("auth_tokens/t1").headers
        val source = GeminiLiveVoiceSource(
            "auth_tokens/t1",
            instructions = "hi",
            endpoint = GeminiLiveSession.Endpoint(wsUrl(), headers),
            device = device,
            main = main,
        )
        val states: MutableList<AgentState> = Collections.synchronizedList(mutableListOf())
        val metrics: MutableList<VoiceMetrics> = Collections.synchronizedList(mutableListOf())
        source.onStateChange { states += it }
        source.onMetrics { metrics += it }
        main.sync { source.connect() }

        // Handshake: the credential went as a header, not in the URL; setup is Web's shape.
        val req = server.takeRequest(5, TimeUnit.SECONDS)!!
        assertEquals("Token auth_tokens/t1", req.headers["Authorization"])
        // LISTENING is emitted inside the same main-thread task that then calls
        // device.start() (handle() -> SetupComplete -> startAudio()). This thread polls
        // from outside that task, so it can see LISTENING before the device is up: the
        // first CI runs (2026-09-21) failed here on `device.started`. Wait for both.
        until { states.lastOrNull() == AgentState.LISTENING && device.started }
        assertTrue(c1.received.first().contains("\"models/gemini-3.8-live\""))
        assertTrue(device.started)
        assertEquals(listOf(AgentState.INITIALIZING, AgentState.LISTENING), states.toList())

        // Mic: a capture-thread chunk becomes one realtimeInput message.
        device.capture!!.invoke(FloatArray(1024) { 0.25f })
        until { c1.received.any { it.contains("realtimeInput") } }

        // A model turn: received -> thinking; played -> speaking; drained + turnComplete -> listening.
        c1.ws!!.send("""{"sessionResumptionUpdate":{"newHandle":"h1","resumable":true}}""")
        val tone = FloatArray(4800) { (0.5 * sin(2 * PI * 440 * it / 24000)).toFloat() }
        val b64 = Pcm.base64Encode(Pcm.floatToPcm16(tone))
        c1.ws!!.send(
            """{"serverContent":{"modelTurn":{"parts":[{"inlineData":{"data":"$b64","mimeType":"audio/pcm;rate=24000"}}]}}}""",
        )
        until { states.lastOrNull() == AgentState.THINKING }
        assertEquals(2400L, device.scheduled.first())
        device.playedFrames = 2400L + 2400
        until { states.lastOrNull() == AgentState.SPEAKING }
        until { (metrics.lastOrNull()?.level ?: 0.0) > 0.05 }
        c1.ws!!.send("""{"serverContent":{"turnComplete":true}}""")
        device.playedFrames = 2400L + 4800
        until { states.lastOrNull() == AgentState.LISTENING }

        // goAway: a new socket resumes with the latest handle.
        c1.ws!!.send("""{"goAway":{"timeLeft":"1s"}}""")
        until { c2.received.any { it.contains("\"setup\"") } }
        assertTrue(c2.received.first().contains(""""handle":"h1""""))
        until { states.takeLast(2) == listOf(AgentState.INITIALIZING, AgentState.LISTENING) }
        assertEquals("reconnect clears playback", 1, device.resets)

        main.sync { source.disconnect() }
        assertEquals(AgentState.IDLE, states.last())
        assertFalse(device.started)
    }

    @Test
    fun setupRejectedByServerGoesToOnErrorAndIdle() {
        server.enqueue(MockResponse().withWebSocketUpgrade(ServerConn(answerSetup = false)))
        val device = FakeDevice()
        val source = GeminiLiveVoiceSource(
            "auth_tokens/expired",
            endpoint = GeminiLiveSession.Endpoint(wsUrl(), emptyMap()),
            device = device,
            main = main,
        )
        val states: MutableList<AgentState> = Collections.synchronizedList(mutableListOf())
        val errors: MutableList<Throwable> = Collections.synchronizedList(mutableListOf())
        source.onStateChange { states += it }
        source.onError { errors += it }
        main.sync { source.connect() }
        assertEquals("Token auth_tokens/expired", server.takeRequest(5, TimeUnit.SECONDS)!!.headers["Authorization"])
        until { errors.isNotEmpty() }
        assertEquals(AgentState.IDLE, states.last())
        assertFalse(device.started)
        assertTrue("a bad credential never opens the mic", device.capture == null)
    }

    @Test
    fun audioStartFailureAfterSetupGoesToOnErrorAndIdle() {
        server.enqueue(MockResponse().withWebSocketUpgrade(ServerConn(answerSetup = true)))
        val device = FakeDevice().apply { failStart = true }
        val source = GeminiLiveVoiceSource(
            "auth_tokens/k",
            endpoint = GeminiLiveSession.Endpoint(wsUrl(), emptyMap()),
            device = device,
            main = main,
        )
        val states: MutableList<AgentState> = Collections.synchronizedList(mutableListOf())
        val errors: MutableList<Throwable> = Collections.synchronizedList(mutableListOf())
        source.onStateChange { states += it }
        source.onError { errors += it }
        main.sync { source.connect() } // returns before setup; the audio starts only after setupComplete
        until { errors.isNotEmpty() }
        assertTrue(errors.single() is SecurityException)
        assertEquals(AgentState.IDLE, states.last())
    }

    /**
     * The gate runs before the socket and before the audio device, so a refused
     * credential never opens either. Mirrors iOS
     * `testRawApiKeyIsRefusedBeforeTheSocketOrThePrompt`.
     */
    @Test
    fun rawApiKeyIsRefusedBeforeTheSocketOrTheDevice() {
        val device = FakeDevice()
        val source = GeminiLiveVoiceSource(
            "AIzaKEY",
            endpoint = GeminiLiveSession.Endpoint(wsUrl(), emptyMap()),
            device = device,
            main = main,
        )
        val states: MutableList<AgentState> = Collections.synchronizedList(mutableListOf())
        source.onStateChange { states += it }
        val err = runCatching { main.sync { source.connect() } }.exceptionOrNull()
        val refused = generateSequence(err) { it.cause }
            .filterIsInstance<dev.sinua.voice.CredentialException>().firstOrNull()
        assertTrue("expected a CredentialException, got $err", refused != null)
        assertTrue("a refusal is never retried", refused!!.fatal)
        assertTrue(refused.message!!, refused.message!!.contains("refusing what looks like a raw, long-lived API key"))
        assertEquals("no socket was opened", 0, server.requestCount)
        assertFalse("the audio device was never started", device.started)
        assertTrue("a refusal never enters initializing", states.isEmpty())
    }

    @Test
    fun missingCredentialThrows() {
        val source = GeminiLiveVoiceSource("  ", device = FakeDevice(), main = main)
        val err = runCatching { main.sync { source.connect() } }.exceptionOrNull()
        assertTrue("$err", err?.cause is dev.sinua.voice.CredentialException)
    }

    /**
     * A token is single-use: every reconnect resumes the session with a NEW token
     * from the provider, the resumption handle intact. Mirrors iOS
     * `testEveryReconnectResumesWithAFreshTokenFromTheProvider` and Web's
     * credentials.test.mjs.
     */
    @Test
    fun everyReconnectResumesWithAFreshTokenFromTheProvider() {
        val c1 = ServerConn(answerSetup = true)
        val c2 = ServerConn(answerSetup = true)
        server.enqueue(MockResponse().withWebSocketUpgrade(c1))
        server.enqueue(MockResponse().withWebSocketUpgrade(c2))
        val minted = java.util.concurrent.atomic.AtomicInteger()
        val source = GeminiLiveVoiceSource(
            dev.sinua.voice.CredentialSource.provider {
                dev.sinua.voice.SinuaCredential("auth_tokens/t${minted.incrementAndGet()}")
            },
            endpointOverride = GeminiLiveSession.Endpoint(wsUrl(), emptyMap()),
            device = FakeDevice(),
            main = main,
        )
        val states: MutableList<AgentState> = Collections.synchronizedList(mutableListOf())
        source.onStateChange { states += it }
        main.sync { source.connect() }
        assertEquals("Token auth_tokens/t1", server.takeRequest(5, TimeUnit.SECONDS)!!.headers["Authorization"])
        until { states.lastOrNull() == AgentState.LISTENING }
        c1.ws!!.send("""{"sessionResumptionUpdate":{"newHandle":"h1","resumable":true}}""")
        c1.ws!!.send("""{"goAway":{"timeLeft":"1s"}}""")
        assertEquals(
            "a fresh token, not the spent one",
            "Token auth_tokens/t2",
            server.takeRequest(5, TimeUnit.SECONDS)!!.headers["Authorization"],
        )
        until { c2.received.any { it.contains("\"setup\"") } }
        assertTrue(c2.received.first().contains(""""handle":"h1""""))
        assertEquals(2, minted.get())
        main.sync { source.disconnect() }
    }

    /** A provider that starts returning a raw key stops the reconnect at once (fatal). */
    @Test
    fun aProviderTurningToARawKeyStopsTheReconnect() {
        val c1 = ServerConn(answerSetup = true)
        server.enqueue(MockResponse().withWebSocketUpgrade(c1))
        val minted = java.util.concurrent.atomic.AtomicInteger()
        val source = GeminiLiveVoiceSource(
            dev.sinua.voice.CredentialSource.provider {
                dev.sinua.voice.SinuaCredential(if (minted.incrementAndGet() == 1) "auth_tokens/ok" else "AIzaRAW")
            },
            endpointOverride = GeminiLiveSession.Endpoint(wsUrl(), emptyMap()),
            device = FakeDevice(),
            main = main,
        )
        val states: MutableList<AgentState> = Collections.synchronizedList(mutableListOf())
        val errors: MutableList<Throwable> = Collections.synchronizedList(mutableListOf())
        source.onStateChange { states += it }
        source.onError { errors += it }
        main.sync { source.connect() }
        until { states.lastOrNull() == AgentState.LISTENING }
        c1.ws!!.send("""{"goAway":{"timeLeft":"1s"}}""")
        until { errors.isNotEmpty() }
        assertTrue((errors.single() as dev.sinua.voice.CredentialException).fatal)
        assertEquals("asked once more, then gave up without retrying", 2, minted.get())
        assertEquals("no socket for a refused credential", 1, server.requestCount)
        assertEquals(AgentState.IDLE, states.last())
    }
}
