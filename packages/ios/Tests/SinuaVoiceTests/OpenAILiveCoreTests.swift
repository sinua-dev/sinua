import XCTest

@testable import SinuaVoice

/// The SDK-free core of the native GPT-Live source: `OpenAILiveSession` against
/// spec/openai-live-cases.json (the table Web and Android read too), `OpenAILiveSignaling`,
/// the credential rule and WARP's calls request. No network, no audio.
final class OpenAILiveCoreTests: XCTestCase {
    private func table() throws -> [String: Any] {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("spec/openai-live-cases.json")
        guard let data = try? Data(contentsOf: url) else {
            throw XCTSkip("spec/openai-live-cases.json not readable at \(url.path) (device run?)")
        }
        return try JSONSerialization.jsonObject(with: data) as! [String: Any]
    }

    func testTheSessionFollowsTheSharedTable() throws {
        let root = try table()
        let k = root["constants"] as! [String: Double]
        XCTAssertEqual(k["speakingLevel"], OpenAILiveSession.speakingLevel)
        XCTAssertEqual(k["speakingTailFrames"], Double(OpenAILiveSession.speakingTailFrames))
        XCTAssertEqual(k["bargeInWindowMs"], OpenAILiveSession.bargeInWindowMs)
        XCTAssertEqual(k["delegationTimeoutMs"], OpenAILiveSession.delegationTimeoutMs)
        let cases = root["cases"] as! [[String: Any]]
        XCTAssertGreaterThanOrEqual(cases.count, 15)
        for c in cases {
            let name = c["name"] as! String
            let s = OpenAILiveSession()
            var interrupts = 0
            s.onInterrupt = { interrupts += 1 }
            s.connecting()
            for (i, step) in (c["steps"] as! [[String: Any]]).enumerated() {
                let at = "\(name), step \(i + 1)"
                let t = (step["t"] as! NSNumber).doubleValue
                if let ev = step["event"] {
                    let text = String(decoding: try JSONSerialization.data(withJSONObject: ev), as: UTF8.self)
                    s.handle(text, now: t)
                } else {
                    let level = (step["level"] as! NSNumber).doubleValue
                    let repeats = (step["repeat"] as? NSNumber)?.intValue ?? 1
                    for n in 0..<repeats { s.tick(level: level, now: t + Double(n) * 33) }
                }
                XCTAssertEqual(s.state.rawValue, step["state"] as? String, at)
                if let n = step["interrupts"] as? Int { XCTAssertEqual(interrupts, n, "\(at): interrupts") }
                if let r = step["closed"] as? String { XCTAssertEqual(s.closedReason, r, "\(at): closed") }
                if let f = step["fatal"] as? String { XCTAssertEqual(s.fatalCode, f, "\(at): fatal") }
            }
        }
    }

    func testSessionRequestShape() throws {
        let url = URL(string: "https://app.example/api/voice/live")!
        let r = OpenAILiveSignaling.sessionRequest(sdpOffer: "v=0\r\noffer", token: "dvf_1", url: url)
        XCTAssertEqual(r.url, url)
        XCTAssertEqual(r.httpMethod, "POST")
        XCTAssertEqual(r.value(forHTTPHeaderField: "Authorization"), "Bearer dvf_1")
        XCTAssertEqual(r.value(forHTTPHeaderField: "Content-Type"), "application/json")
        let body = try JSONSerialization.jsonObject(with: r.httpBody!) as! [String: String]
        XCTAssertEqual(body, ["sdp": "v=0\r\noffer"])
        let anonymous = OpenAILiveSignaling.sessionRequest(sdpOffer: "v=0", token: nil, url: url)
        XCTAssertNil(anonymous.value(forHTTPHeaderField: "Authorization"))
    }

    func testAnswerTakesOpenAIsJsonOrBareSdp() throws {
        let json = Data(#"{"session":{"id":"live_1"},"transport":{"type":"webrtc","sdp":"v=0 a"}}"#.utf8)
        let a = try OpenAILiveSignaling.answer(status: 201, body: json)
        XCTAssertEqual(a.sdp, "v=0 a")
        XCTAssertEqual(a.sessionId, "live_1")
        let bare = try OpenAILiveSignaling.answer(status: 200, body: Data("v=0 b".utf8))
        XCTAssertEqual(bare.sdp, "v=0 b")
        XCTAssertNil(bare.sessionId)
        XCTAssertThrowsError(try OpenAILiveSignaling.answer(status: 200, body: Data("{}".utf8)))
        XCTAssertThrowsError(try OpenAILiveSignaling.answer(status: 403, body: Data("no".utf8))) {
            XCTAssertEqual($0 as? OpenAIRealtimeSignaling.SignalingError, .fatal(status: 403, body: "no"))
        }
    }

    func testLiveCredentialRule() {
        let own = URL(string: "https://app.example/api/voice/live")!
        let openai = URL(string: "https://api.openai.com/v1/live/sessions")!
        XCTAssertNil(InsecureCredential.openAILiveRefusal(sessionURL: own, credential: nil))
        XCTAssertNil(InsecureCredential.openAILiveRefusal(sessionURL: own, credential: "dvf_x"))
        XCTAssertNotNil(InsecureCredential.openAILiveRefusal(sessionURL: own, credential: " sk-proj-x"))
        XCTAssertEqual(
            InsecureCredential.openAILiveRefusal(sessionURL: openai, credential: nil),
            "OpenAILiveVoiceSource: sessionUrl must be your own endpoint; GPT-Live sessions are opened by your "
                + "server with its key (POST /v1/live/sessions), never from the app.")
        XCTAssertThrowsError(try InsecureCredential.checkOpenAILive(sessionURL: openai, credential: nil))
    }

    func testWarpCallsRequestCarriesTheChannelId() {
        let plain = OpenAIRealtimeSignaling.callsRequest(sdpOffer: "v=0", ephemeralKey: "ek_1")
        XCTAssertEqual(plain.url, OpenAIRealtimeSignaling.callsURL)
        let warp = OpenAIRealtimeSignaling.callsRequest(
            sdpOffer: "v=0", ephemeralKey: "ek_1",
            url: URL(string: "https://app.example/calls?model=gpt-realtime")!,
            dcid: OpenAIRealtimeSignaling.warpDataChannelId)
        XCTAssertEqual(warp.url?.absoluteString, "https://app.example/calls?model=gpt-realtime&dcid=1")
        XCTAssertEqual(
            OpenAIRealtimeSignaling.warpFieldTrials,
            "WebRTC-ForceDtls13/Enabled/WebRTC-Sctp-Snap/Enabled/WebRTC-IceHandshakeDtls/Enabled/")
    }
}
