import Network
import SinuaVoice
import XCTest

@testable import SinuaGeminiLive

/// The real `GeminiLiveVoiceSource` + `URLSessionWebSocketTask` against an
/// in-process fake Gemini Live server (Network.framework WebSocket on
/// localhost). The audio device is faked so the playback clock is driven by
/// the test. Checks the wire, not the vendor: the live API is untested here.
@MainActor
final class GeminiLiveVoiceSourceTests: XCTestCase {
    func testHandshakeMicTurnGoAwayResumeAndDisconnect() async throws {
        let server = try FakeGeminiServer()
        try await server.ready()
        let token = GeminiLiveSession.endpoint(credential: "auth_tokens/t1")
        let endpoint = GeminiLiveSession.Endpoint(
            url: URL(string: "ws://127.0.0.1:\(server.port)/live")!, headers: token.headers)
        let device = FakeDevice()
        let source = GeminiLiveVoiceSource(
            credential: "auth_tokens/t1", instructions: "hi", endpoint: endpoint,
            device: device, requestPermission: { true })
        var states: [AgentState] = []
        var metrics: [VoiceMetrics] = []
        source.onStateChange { states.append($0) }
        source.onMetrics { metrics.append($0) }

        server.onMessage = { conn, text in
            if text.contains("\"setup\"") { server.send(#"{"setupComplete":{}}"#, on: conn) }
        }
        try await source.connect()

        // Handshake: the credential went as a header, not in the URL; setup is Web's shape.
        XCTAssertEqual(server.requestHeaders.first?["authorization"], "Token auth_tokens/t1")
        let setup = try XCTUnwrap(server.received.first { $0.contains("\"setup\"") })
        XCTAssertTrue(setup.contains("\"models/gemini-3.8-live\""))
        XCTAssertTrue(device.started)
        XCTAssertEqual(states, [.initializing, .listening])

        // Mic: an audio-thread chunk becomes one realtimeInput message.
        device.onCapture?([Float](repeating: 0.25, count: 1024))
        try await until { server.received.contains { $0.contains("realtimeInput") } }

        // A model turn: received -> thinking; played -> speaking; drained + turnComplete -> listening.
        let conn1 = try XCTUnwrap(server.connections.first)
        server.send(#"{"sessionResumptionUpdate":{"newHandle":"h1","resumable":true}}"#, on: conn1)
        let tone = (0..<4800).map { Float(0.5 * sin(2 * .pi * 440 * Double($0) / 24000)) }
        let b64 = Pcm.floatToPcm16(tone).base64EncodedString()
        server.send(
            #"{"serverContent":{"modelTurn":{"parts":[{"inlineData":{"data":"\#(b64)","mimeType":"audio/pcm;rate=24000"}}]}}}"#,
            on: conn1)
        try await until { states.last == .thinking }
        XCTAssertEqual(device.scheduled.first?.frame, 2400)
        device.playedFrames = 2400 + 2400
        try await until { states.last == .speaking }
        try await until { (metrics.last?.level ?? 0) > 0.05 }
        server.send(#"{"serverContent":{"turnComplete":true}}"#, on: conn1)
        device.playedFrames = 2400 + 4800
        try await until { states.last == .listening }

        // goAway: a new socket resumes with the latest handle.
        server.onMessage = { conn, text in
            if text.contains("\"setup\"") { server.send(#"{"setupComplete":{}}"#, on: conn) }
        }
        server.send(#"{"goAway":{"timeLeft":"1s"}}"#, on: conn1)
        try await until {
            server.connections.count == 2 && server.received.filter { $0.contains("\"setup\"") }.count == 2
        }
        let resumed = try XCTUnwrap(server.received.last { $0.contains("\"setup\"") })
        XCTAssertTrue(resumed.contains(#""handle":"h1""#), resumed)
        try await until { states.suffix(2) == [.initializing, .listening] }
        XCTAssertEqual(device.resets, 1, "reconnect clears playback")

        source.disconnect()
        XCTAssertEqual(states.last, .idle)
        XCTAssertFalse(device.started)
        server.stop()
    }

    func testSetupRejectedByServerFailsConnect() async throws {
        let server = try FakeGeminiServer()
        try await server.ready()
        server.onMessage = { conn, _ in conn.cancel() }  // a bad key: close instead of setupComplete
        let device = FakeDevice()
        var asked = 0
        let source = GeminiLiveVoiceSource(
            credential: "AIzaKEY",
            allowInsecureApiKey: true,  // a raw key is the dev path; the gate is tested below
            endpoint: .init(
                url: URL(string: "ws://127.0.0.1:\(server.port)/")!,
                headers: GeminiLiveSession.endpoint(credential: "AIzaKEY").headers),
            device: device,
            requestPermission: {
                asked += 1
                return true
            })
        var states: [AgentState] = []
        source.onStateChange { states.append($0) }
        do {
            try await source.connect()
            XCTFail("expected a setup failure")
        } catch let e as GeminiLiveError {
            if case .closedDuringSetup = e {} else { XCTFail("\(e)") }
        }
        XCTAssertEqual(server.requestHeaders.first?["x-goog-api-key"], "AIzaKEY")
        XCTAssertEqual(states.last, .idle)
        XCTAssertEqual(asked, 0, "a bad credential fails before any permission prompt")
        XCTAssertFalse(device.started, "...and the mic/audio never started")
        XCTAssertNil(device.onCapture)
        server.stop()
    }

    func testPermissionDeniedAfterSetupClosesTheSocketAndNeverStartsAudio() async throws {
        let server = try FakeGeminiServer()
        try await server.ready()
        server.onMessage = { conn, text in
            if text.contains("\"setup\"") { server.send(#"{"setupComplete":{}}"#, on: conn) }
        }
        let device = FakeDevice()
        let source = GeminiLiveVoiceSource(
            credential: "k", allowInsecureApiKey: true,
            endpoint: .init(url: URL(string: "ws://127.0.0.1:\(server.port)/")!, headers: [:]),
            device: device, requestPermission: { false })
        do {
            try await source.connect()
            XCTFail("expected permissionDenied")
        } catch let e as VoiceSourceError { XCTAssertEqual(e, .permissionDenied) }
        XCTAssertFalse(device.started)
        XCTAssertEqual(server.received.filter { $0.contains("\"setup\"") }.count, 1, "authenticated first")
        server.stop()
    }

    /// The gate runs before the socket and before the permission prompt, so a
    /// refused credential never opens a device or a connection.
    func testRawApiKeyIsRefusedBeforeTheSocketOrThePrompt() async throws {
        let server = try FakeGeminiServer()
        try await server.ready()
        var asked = 0
        let source = GeminiLiveVoiceSource(
            credential: "AIzaKEY",
            endpoint: .init(url: URL(string: "ws://127.0.0.1:\(server.port)/")!, headers: [:]),
            device: FakeDevice(),
            requestPermission: {
                asked += 1
                return true
            })
        var states: [AgentState] = []
        source.onStateChange { states.append($0) }
        do {
            try await source.connect()
            XCTFail("expected the raw key to be refused")
        } catch let e as InsecureCredential.Refused {
            XCTAssertTrue(e.message.contains("refusing a raw, long-lived API key"), e.message)
        }
        XCTAssertEqual(asked, 0, "no permission prompt")
        XCTAssertTrue(server.received.isEmpty, "no socket traffic")
        XCTAssertTrue(states.isEmpty, "a refusal never enters initializing")
        server.stop()
    }

    func testMissingCredential() async throws {
        do {
            try await GeminiLiveVoiceSource(credential: "  ", device: FakeDevice(), requestPermission: { true })
                .connect()
            XCTFail("expected missingCredential")
        } catch let e as GeminiLiveError { XCTAssertEqual(e, .missingCredential) }
    }

    private func until(_ timeout: TimeInterval = 5, _ cond: () -> Bool) async throws {
        let end = Date().addingTimeInterval(timeout)
        while !cond() {
            if Date() > end { throw TimeoutError() }
            try await Task.sleep(nanoseconds: 10_000_000)
        }
    }

    struct TimeoutError: Error {}
}

final class FakeDevice: PcmAudioDevice {
    var playedFrames: Int64 = 0
    var scheduled: [(frame: Int64, count: Int)] = []
    var resets = 0
    var started = false
    var onCapture: (@Sendable ([Float]) -> Void)?
    var onClockReset: (() -> Void)?

    func start(inputRate _: Int, outputRate _: Int, onCapture: @escaping @Sendable ([Float]) -> Void) throws {
        started = true
        self.onCapture = onCapture
    }

    func schedule(_ samples: [Float], atFrame frame: Int64) { scheduled.append((frame, samples.count)) }

    func resetPlayback(fade _: Bool) {
        resets += 1
        playedFrames = 0
    }

    func stop() { started = false }
}

final class HeaderBox: @unchecked Sendable {
    var all: [[String: String]] = []
}

/// A WebSocket server on 127.0.0.1 (random port) that records the upgrade
/// headers and every text frame. Callbacks and state on the main thread.
@MainActor
final class FakeGeminiServer {
    private let listener: NWListener
    private(set) var port: UInt16 = 0
    private let headerBox: HeaderBox
    var requestHeaders: [[String: String]] { headerBox.all }
    private(set) var received: [String] = []
    private(set) var connections: [NWConnection] = []
    var onMessage: ((NWConnection, String) -> Void)?

    init() throws {
        let ws = NWProtocolWebSocket.Options()
        ws.autoReplyPing = true
        let params = NWParameters.tcp
        params.defaultProtocolStack.applicationProtocols.insert(ws, at: 0)
        params.requiredLocalEndpoint = .hostPort(host: "127.0.0.1", port: .any)
        // Must be set before the listener copies the options. Header names lowercased.
        let box = HeaderBox()
        headerBox = box
        ws.setClientRequestHandler(.main) { _, headers in
            box.all.append(
                Dictionary(headers.map { ($0.name.lowercased(), $0.value) }, uniquingKeysWith: { a, _ in a }))
            return .init(status: .accept, subprotocol: nil)
        }
        listener = try NWListener(using: params)
        listener.newConnectionHandler = { [weak self] conn in
            MainActor.assumeIsolated {
                guard let self else { return }
                self.connections.append(conn)
                conn.start(queue: .main)
                self.receive(conn)
            }
        }
    }

    func ready() async throws {
        try await withCheckedThrowingContinuation { (cont: CheckedContinuation<Void, Error>) in
            var done = false
            listener.stateUpdateHandler = { [weak self] state in
                MainActor.assumeIsolated {
                    guard !done else { return }
                    switch state {
                    case .ready:
                        done = true
                        self?.port = self?.listener.port?.rawValue ?? 0
                        cont.resume()
                    case .failed(let e):
                        done = true
                        cont.resume(throwing: e)
                    default: break
                    }
                }
            }
            listener.start(queue: .main)
        }
    }

    func send(_ text: String, on conn: NWConnection) {
        let meta = NWProtocolWebSocket.Metadata(opcode: .text)
        let ctx = NWConnection.ContentContext(identifier: "text", metadata: [meta])
        conn.send(
            content: Data(text.utf8), contentContext: ctx, isComplete: true, completion: .contentProcessed { _ in })
    }

    func stop() {
        for connection in connections { connection.cancel() }
        listener.cancel()
    }

    private func receive(_ conn: NWConnection) {
        conn.receiveMessage { [weak self] data, _, _, error in
            MainActor.assumeIsolated {
                guard let self, error == nil else { return }
                if let data, let text = String(data: data, encoding: .utf8) {
                    self.received.append(text)
                    self.onMessage?(conn, text)
                }
                self.receive(conn)
            }
        }
    }
}
