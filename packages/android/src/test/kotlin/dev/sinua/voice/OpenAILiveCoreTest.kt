package dev.sinua.voice

import org.json.JSONObject
import org.junit.Assert.assertEquals
import org.junit.Assert.assertNotNull
import org.junit.Assert.assertNull
import org.junit.Assert.assertThrows
import org.junit.Assert.assertTrue
import org.junit.Test
import java.io.File

/**
 * The SDK-free core of the native GPT-Live source: [OpenAILiveSession] against
 * spec/openai-live-cases.json (the table Web and iOS read too), [OpenAILiveSignaling], the
 * credential rule and WARP's calls request. Plain JVM: no Android, no network, no audio.
 */
class OpenAILiveCoreTest {
    @Test fun theSessionFollowsTheSharedTable() {
        val root = JSONObject(File("../../spec/openai-live-cases.json").readText())
        val k = root.getJSONObject("constants")
        assertEquals(OpenAILiveSession.SPEAKING_LEVEL, k.getDouble("speakingLevel"), 0.0)
        assertEquals(OpenAILiveSession.SPEAKING_TAIL_FRAMES, k.getInt("speakingTailFrames"))
        assertEquals(OpenAILiveSession.SPEAKING_TAIL_PER_SECOND, k.getDouble("speakingTailPerSecond"), 0.0)
        assertEquals(OpenAILiveSession.SPEAKING_TAIL_MAX_FRAMES, k.getInt("speakingTailMaxFrames"))
        assertEquals(OpenAILiveSession.BARGE_IN_TAIL_FRAMES, k.getInt("bargeInTailFrames"))
        assertEquals(OpenAILiveSession.BARGE_IN_WINDOW_MS, k.getDouble("bargeInWindowMs"), 0.0)
        assertEquals(OpenAILiveSession.DELEGATION_TIMEOUT_MS, k.getDouble("delegationTimeoutMs"), 0.0)
        val cases = root.getJSONArray("cases")
        assertTrue(cases.length() >= 15)
        for (c in 0 until cases.length()) {
            val case = cases.getJSONObject(c)
            val name = case.getString("name")
            val s = OpenAILiveSession()
            var interrupts = 0
            s.onInterrupt = { interrupts++ }
            s.connecting()
            val steps = case.getJSONArray("steps")
            for (i in 0 until steps.length()) {
                val step = steps.getJSONObject(i)
                val at = "$name, step ${i + 1}"
                val t = step.getDouble("t")
                if (step.has("event")) {
                    s.handle(step.getJSONObject("event").toString(), t)
                } else {
                    val level = step.getDouble("level")
                    repeat(step.optInt("repeat", 1)) { n -> s.tick(level, t + n * 33) }
                }
                assertEquals(at, step.getString("state"), s.state.wire)
                if (step.has("interrupts")) assertEquals("$at: interrupts", step.getInt("interrupts"), interrupts)
                if (step.has("closed")) assertEquals("$at: closed", step.getString("closed"), s.closedReason)
                if (step.has("fatal")) assertEquals("$at: fatal", step.getString("fatal"), s.fatalCode)
            }
        }
    }

    @Test fun sessionRequestShape() {
        val url = "https://app.example/api/voice/live"
        val r = OpenAILiveSignaling.sessionRequest("v=0\r\noffer", "dvf_1", url)
        assertEquals(url, r.url)
        assertEquals("Bearer dvf_1", r.headers["Authorization"])
        assertEquals("application/json", r.contentType)
        assertEquals("v=0\r\noffer", JSONObject(r.body).getString("sdp"))
        assertNull(OpenAILiveSignaling.sessionRequest("v=0", null, url).headers["Authorization"])
    }

    @Test fun answerTakesOpenAIsJsonOrBareSdp() {
        val a = OpenAILiveSignaling.answer(201, """{"session":{"id":"live_1"},"transport":{"type":"webrtc","sdp":"v=0 a"}}""")
        assertEquals(OpenAILiveSignaling.Answer("v=0 a", "live_1"), a)
        assertEquals(OpenAILiveSignaling.Answer("v=0 b", null), OpenAILiveSignaling.answer(200, "v=0 b"))
        assertThrows(OpenAIRealtimeSignaling.SignalingException.Malformed::class.java) {
            OpenAILiveSignaling.answer(200, "{}")
        }
        assertThrows(OpenAIRealtimeSignaling.SignalingException.Fatal::class.java) {
            OpenAILiveSignaling.answer(403, "no")
        }
    }

    @Test fun liveCredentialRule() {
        val own = "https://app.example/api/voice/live"
        val openai = "https://api.openai.com/v1/live/sessions"
        assertNull(InsecureCredential.openAILiveRefusal(own, null))
        assertNull(InsecureCredential.openAILiveRefusal(own, "dvf_x"))
        assertNotNull(InsecureCredential.openAILiveRefusal(own, " sk-proj-x"))
        assertEquals(
            "OpenAILiveVoiceSource: sessionUrl must be your own endpoint; GPT-Live sessions are opened by your " +
                "server with its key (POST /v1/live/sessions), never from the app.",
            InsecureCredential.openAILiveRefusal(openai, null),
        )
        val e = assertThrows(CredentialException::class.java) { InsecureCredential.checkOpenAILive(openai, null) }
        assertTrue(e.fatal)
    }

    @Test fun warpCallsRequestCarriesTheChannelId() {
        assertEquals(OpenAIRealtimeSignaling.CALLS_URL, OpenAIRealtimeSignaling.callsRequest("v=0", "ek_1").url)
        val warp = OpenAIRealtimeSignaling.callsRequest(
            "v=0",
            "ek_1",
            "https://app.example/calls?model=gpt-realtime",
            OpenAIRealtimeSignaling.WARP_DATA_CHANNEL_ID,
        )
        assertEquals("https://app.example/calls?model=gpt-realtime&dcid=1", warp.url)
        assertEquals(
            "https://api.openai.com/v1/realtime/calls?dcid=1",
            OpenAIRealtimeSignaling.callsRequest("v=0", "ek_1", dcid = 1).url,
        )
        assertEquals(
            "WebRTC-ForceDtls13/Enabled/WebRTC-Sctp-Snap/Enabled/WebRTC-IceHandshakeDtls/Enabled/",
            OpenAIRealtimeSignaling.WARP_FIELD_TRIALS,
        )
    }
}
