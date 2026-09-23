import XCTest

@testable import SinuaVoice

/// The SDK-free core of the native OpenAI Realtime source: `OpenAIRealtimeSession`
/// (WebRTCVoiceSource.ts's event + tick rules), `RealtimeReconnect` / `TranscriptLog`
/// (realtimeReconnect.ts), and `OpenAIRealtimeSignaling` through a URLProtocol stub
/// (no network).
final class OpenAIRealtimeCoreTests: XCTestCase {
    final class Box {
        var states: [AgentState] = []
        var interrupts = 0
    }

    private func make() -> (OpenAIRealtimeSession, Box) {
        let s = OpenAIRealtimeSession()
        let box = Box()
        s.onState = { box.states.append($0) }
        s.onInterrupt = { box.interrupts += 1 }
        s.connecting()
        s.connected()
        return (s, box)
    }

    private func ev(_ type: String, _ extra: String = "") -> String { #"{"type":"\#(type)"\#(extra)}"# }

    // MARK: - Session

    func testVadAndResponseLifecycleWithOutputBufferEvents() {
        let (s, box) = make()
        s.handle(ev("input_audio_buffer.speech_started"))
        s.handle(ev("input_audio_buffer.speech_stopped"))
        s.handle(ev("response.created"))
        s.handle(ev("output_audio_buffer.started"))
        s.tick(level: 0)  // energy doesn't matter once the buffer events exist
        s.handle(ev("response.done"))
        XCTAssertEqual(s.state, .speaking, "done while audible: stopped will follow")
        s.handle(ev("output_audio_buffer.stopped"))
        XCTAssertEqual(box.states, [.initializing, .listening, .thinking, .speaking, .listening])
        XCTAssertEqual(box.interrupts, 0)
    }

    func testBargeInOnlyOverAudibleOutput() {
        let (s, box) = make()
        s.handle(ev("response.created"))
        s.handle(ev("input_audio_buffer.speech_started"))
        XCTAssertEqual(box.interrupts, 0, "speech over thinking isn't a barge-in")
        s.handle(ev("response.created"))
        s.handle(ev("output_audio_buffer.started"))
        s.handle(ev("input_audio_buffer.speech_started"))
        XCTAssertEqual(box.interrupts, 1)
        s.handle(ev("output_audio_buffer.cleared"))
        XCTAssertEqual(s.state, .listening)
    }

    func testResponseWithoutAudioDoesNotHang() {
        let (s, _) = make()
        s.handle(ev("response.created"))
        s.handle(ev("response.done"))
        XCTAssertEqual(s.state, .listening)
    }

    func testEnergyFallbackWithoutOutputBufferEvents() {
        let (s, _) = make()
        s.handle(ev("response.created"))
        s.tick(level: 0.2)
        XCTAssertEqual(s.state, .speaking, "energy during a response")
        s.handle(ev("response.done"))
        for _ in 0..<8 { s.tick(level: 0.01) }
        XCTAssertEqual(s.state, .speaking, "the ~300 ms tail")
        s.tick(level: 0.01)
        XCTAssertEqual(s.state, .listening)
        s.tick(level: 0.3)
        XCTAssertEqual(s.state, .listening, "no response active: energy alone doesn't speak")
    }

    func testTranscriptAndFatalErrors() {
        let (s, _) = make()
        s.handle(ev("response.output_audio_transcript.done", ##","transcript":" Hello there ""##))
        s.handle(ev("conversation.item.input_audio_transcription.completed", ##","transcript":"hi""##))
        s.handle(ev("response.output_audio_transcript.done", ##","transcript":"   ""##))
        XCTAssertEqual(
            s.transcript.window(), [.init(role: .assistant, text: "Hello there"), .init(role: .user, text: "hi")])
        s.handle(ev("error", #","error":{"code":"rate_limit_exceeded"}"#))
        XCTAssertNil(s.fatalCode)
        s.handle(ev("error", #","error":{"code":"invalid_api_key"}"#))
        XCTAssertEqual(s.fatalCode, "invalid_api_key")
        s.connecting()
        XCTAssertNil(s.fatalCode, "a new session starts clean")
        s.handle("nope")
        s.stopped()
        XCTAssertEqual(s.state, .idle)
    }

    // MARK: - Reconnect

    func testBackoffDelaysLikeWeb() {
        XCTAssertEqual(RealtimeReconnect.delayMs(attempt: 1, random: 0.5), 100)
        XCTAssertEqual(RealtimeReconnect.delayMs(attempt: 2, random: 0.5), 1000)
        XCTAssertEqual(RealtimeReconnect.delayMs(attempt: 3, random: 0.5), 2000)
        XCTAssertEqual(RealtimeReconnect.delayMs(attempt: 9, random: 0.5), 8000, "capped")
        XCTAssertEqual(RealtimeReconnect.delayMs(attempt: 2, random: 0), 800, "-20% jitter")
        XCTAssertEqual(RealtimeReconnect.delayMs(attempt: 2, random: 0.999_999), 1200, "+20% jitter")
        XCTAssertTrue(RealtimeReconnect.isRetryable(httpStatus: 429))
        XCTAssertTrue(RealtimeReconnect.isRetryable(httpStatus: 503))
        XCTAssertFalse(RealtimeReconnect.isRetryable(httpStatus: 401))
        XCTAssertTrue(RealtimeReconnect.isFatalError(code: "insufficient_quota"))
        XCTAssertFalse(RealtimeReconnect.isFatalError(code: nil))
    }

    func testTranscriptReplayWindowAndEvents() {
        let log = TranscriptLog(maxChars: 12, maxItems: 2)
        log.add(.user, "aaaa")
        log.add(.assistant, "bbbbbbbb")
        log.add(.user, "cccc")
        XCTAssertEqual(log.window().map(\.text), ["bbbbbbbb", "cccc"], "newest within both budgets")
        let first = try! JSONSerialization.jsonObject(with: Data(log.replayEvents()[0].utf8)) as! [String: Any]
        XCTAssertEqual(first["type"] as? String, "conversation.item.create")
        let item = first["item"] as! [String: Any]
        XCTAssertEqual(item["role"] as? String, "assistant")
        XCTAssertEqual((item["content"] as! [[String: String]])[0], ["type": "output_text", "text": "bbbbbbbb"])
        let second = try! JSONSerialization.jsonObject(with: Data(log.replayEvents()[1].utf8)) as! [String: Any]
        XCTAssertEqual(((second["item"] as! [String: Any])["content"] as! [[String: String]])[0]["type"], "input_text")
    }

    // MARK: - Signaling (URLProtocol stub, no network)

    final class Stub: URLProtocol {
        nonisolated(unsafe) static var status = 201
        nonisolated(unsafe) static var body = Data()
        nonisolated(unsafe) static var seen: [URLRequest] = []
        nonisolated(unsafe) static var seenBodies: [Data] = []

        override class func canInit(with request: URLRequest) -> Bool { true }
        override class func canonicalRequest(for request: URLRequest) -> URLRequest { request }
        override func startLoading() {
            Stub.seen.append(request)
            // URLSession moves httpBody into a stream for protocol handlers.
            var body = request.httpBody ?? Data()
            if body.isEmpty, let s = request.httpBodyStream {
                s.open()
                var buf = [UInt8](repeating: 0, count: 65536)
                while s.hasBytesAvailable {
                    let n = s.read(&buf, maxLength: buf.count)
                    if n <= 0 { break }
                    body.append(buf, count: n)
                }
                s.close()
            }
            Stub.seenBodies.append(body)
            let resp = HTTPURLResponse(
                url: request.url!, statusCode: Stub.status, httpVersion: "HTTP/1.1", headerFields: nil)!
            client?.urlProtocol(self, didReceive: resp, cacheStoragePolicy: .notAllowed)
            client?.urlProtocol(self, didLoad: Stub.body)
            client?.urlProtocolDidFinishLoading(self)
        }
        override func stopLoading() {}
    }

    private func stubbedSession() -> URLSession {
        let c = URLSessionConfiguration.ephemeral
        c.protocolClasses = [Stub.self]
        return URLSession(configuration: c)
    }

    func testCallsRequestShapeAndAnswer() async throws {
        Stub.seen = []
        Stub.seenBodies = []
        Stub.status = 201
        Stub.body = Data("v=0\r\no=- 1 2 IN IP4 0.0.0.0\r\n".utf8)
        let req = OpenAIRealtimeSignaling.callsRequest(sdpOffer: "v=0\r\noffer", ephemeralKey: "ek_123")
        let (status, body) = try await OpenAIRealtimeSignaling.send(req, session: stubbedSession())
        XCTAssertEqual(
            try OpenAIRealtimeSignaling.answer(status: status, body: body), "v=0\r\no=- 1 2 IN IP4 0.0.0.0\r\n")
        let sent = try XCTUnwrap(Stub.seen.first)
        XCTAssertEqual(sent.url, OpenAIRealtimeSignaling.callsURL)
        XCTAssertEqual(sent.httpMethod, "POST")
        XCTAssertEqual(sent.value(forHTTPHeaderField: "Authorization"), "Bearer ek_123")
        XCTAssertEqual(sent.value(forHTTPHeaderField: "Content-Type"), "application/sdp")
        XCTAssertEqual(String(decoding: Stub.seenBodies[0], as: UTF8.self), "v=0\r\noffer")
    }

    func testStatusMapping() {
        XCTAssertThrowsError(try OpenAIRealtimeSignaling.answer(status: 401, body: Data("no".utf8))) {
            XCTAssertEqual($0 as? OpenAIRealtimeSignaling.SignalingError, .fatal(status: 401, body: "no"))
        }
        XCTAssertThrowsError(try OpenAIRealtimeSignaling.answer(status: 503, body: Data())) {
            XCTAssertEqual($0 as? OpenAIRealtimeSignaling.SignalingError, .retryable(status: 503, body: ""))
        }
        XCTAssertThrowsError(try OpenAIRealtimeSignaling.answer(status: 200, body: Data("{}".utf8)))
    }

    func testDevClientSecretRequestAndParse() async throws {
        Stub.seen = []
        Stub.seenBodies = []
        Stub.status = 200
        Stub.body = Data(#"{"value":"ek_abc","expires_at":1}"#.utf8)
        let req = OpenAIRealtimeSignaling.clientSecretRequest(apiKey: "sk-dev", instructions: "Be brief.")
        let (status, body) = try await OpenAIRealtimeSignaling.send(req, session: stubbedSession())
        XCTAssertEqual(try OpenAIRealtimeSignaling.clientSecret(status: status, body: body), "ek_abc")
        XCTAssertEqual(Stub.seen.first?.value(forHTTPHeaderField: "Authorization"), "Bearer sk-dev")
        let json = try JSONSerialization.jsonObject(with: Stub.seenBodies[0]) as! [String: Any]
        let session = json["session"] as! [String: Any]
        XCTAssertEqual(session["model"] as? String, "gpt-realtime")
        XCTAssertEqual(session["instructions"] as? String, "Be brief.")
        XCTAssertEqual(
            ((session["audio"] as! [String: Any])["input"] as! [String: Any])["turn_detection"] as? [String: String],
            ["type": "server_vad"])
        XCTAssertEqual((json["expires_after"] as! [String: Any])["seconds"] as? Int, 600)
        XCTAssertThrowsError(try OpenAIRealtimeSignaling.clientSecret(status: 200, body: Data("{}".utf8)))
    }
}
