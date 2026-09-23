package dev.sinua.openai

import dev.sinua.voice.OpenAIRealtimeSignaling
import kotlinx.coroutines.runBlocking
import okhttp3.mockwebserver.MockResponse
import okhttp3.mockwebserver.MockWebServer
import org.junit.After
import org.junit.Assert.assertEquals
import org.junit.Assert.assertTrue
import org.junit.Before
import org.junit.Test

/**
 * The signaling HTTP path with real OkHttp against an in-process server: the calls
 * POST (headers, exact content type, SDP body, answer text, status mapping) and the
 * dev client_secrets POST. No WebRTC, no audio.
 */
class OpenAIHttpTest {
    private lateinit var server: MockWebServer

    @Before
    fun setUp() {
        server = MockWebServer()
        server.start()
    }

    @After
    fun tearDown() {
        server.shutdown()
    }

    @Test
    fun callsPostAndAnswer() = runBlocking {
        server.enqueue(MockResponse().setResponseCode(201).setBody("v=0\r\no=- 1 2 IN IP4 0.0.0.0\r\n"))
        val url = server.url("/v1/realtime/calls").toString()
        val (status, body) = OpenAIHttp().send(OpenAIRealtimeSignaling.callsRequest("v=0\r\noffer", "ek_123", url))
        assertEquals("v=0\r\no=- 1 2 IN IP4 0.0.0.0\r\n", OpenAIRealtimeSignaling.answer(status, body))
        val req = server.takeRequest()
        assertEquals("POST", req.method)
        assertEquals("/v1/realtime/calls", req.path)
        assertEquals("Bearer ek_123", req.getHeader("Authorization"))
        assertEquals("application/sdp", req.getHeader("Content-Type"))
        assertEquals("v=0\r\noffer", req.body.readUtf8())
    }

    @Test
    fun statusesMapToFatalAndRetryable() = runBlocking {
        server.enqueue(MockResponse().setResponseCode(401).setBody("bad key"))
        server.enqueue(MockResponse().setResponseCode(503))
        val url = server.url("/c").toString()
        val (s1, b1) = OpenAIHttp().send(OpenAIRealtimeSignaling.callsRequest("v=0", "ek_x", url))
        assertTrue(
            runCatching {
                OpenAIRealtimeSignaling.answer(s1, b1)
            }.exceptionOrNull() is OpenAIRealtimeSignaling.SignalingException.Fatal,
        )
        val (s2, b2) = OpenAIHttp().send(OpenAIRealtimeSignaling.callsRequest("v=0", "ek_x", url))
        assertTrue(
            runCatching {
                OpenAIRealtimeSignaling.answer(s2, b2)
            }.exceptionOrNull() is OpenAIRealtimeSignaling.SignalingException.Retryable,
        )
    }

    @Test
    fun devClientSecretPost() = runBlocking {
        server.enqueue(MockResponse().setResponseCode(200).setBody("""{"value":"ek_abc","expires_at":1}"""))
        val url = server.url("/v1/realtime/client_secrets").toString()
        val (status, body) = OpenAIHttp().send(OpenAIRealtimeSignaling.clientSecretRequest("sk-dev", url = url))
        assertEquals("ek_abc", OpenAIRealtimeSignaling.clientSecret(status, body))
        val req = server.takeRequest()
        assertEquals("Bearer sk-dev", req.getHeader("Authorization"))
        assertTrue(req.getHeader("Content-Type")!!.startsWith("application/json"))
        assertTrue(req.body.readUtf8().contains("\"server_vad\""))
    }
}
