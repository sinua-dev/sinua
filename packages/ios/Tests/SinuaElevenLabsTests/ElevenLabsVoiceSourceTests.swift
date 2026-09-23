import Network
import SinuaVoice
import XCTest

@testable import SinuaElevenLabs

/// The real `ElevenLabsVoiceSource` + `URLSessionWebSocketTask` against an
/// in-process fake ElevenLabs server (Network.framework WebSocket on
/// localhost, negotiating the `convai` subprotocol). The audio device is faked:
/// no microphone, no sound; the playback clock is driven by the test.
@MainActor
final class ElevenLabsVoiceSourceTests: XCTestCase {
    private let metadata =
        #"{"type":"conversation_initiation_metadata","conversation_initiation_metadata_event":{"conversation_id":"c1","agent_output_audio_format":"pcm_24000","user_input_audio_format":"pcm_16000"}}"#

    private func audio(id: Int, count: Int = 4800) -> String {
        let tone = (0..<count).map { Float(0.5 * sin(2 * .pi * 440 * Double($0) / 24000)) }
        return
            #"{"type":"audio","audio_event":{"audio_base_64":"\#(Pcm.floatToPcm16(tone).base64EncodedString())","event_id":\#(id)}}"#
    }

    func testHandshakeFormatsTurnPingInterruptionAndAgentEnd() async throws {
        let server = try FakeWSServer(subprotocol: "convai")
        try await server.ready()
        let device = FakeDevice()
        let source = ElevenLabsVoiceSource(
            credential: "agent_abc", overrides: ["agent": ["language": "tr"]],
            endpoint: URL(string: "ws://127.0.0.1:\(server.port)/v1/convai/conversation?agent_id=agent_abc")!,
            device: device, requestPermission: { true })
        var states: [AgentState] = []
        var metrics: [VoiceMetrics] = []
        var interrupts = 0
        source.onStateChange { states.append($0) }
        source.onMetrics { metrics.append($0) }
        source.onInterrupt { interrupts += 1 }
        server.onMessage = { conn, text in
            if text.contains("conversation_initiation_client_data") { server.send(self.metadata, on: conn) }
        }
        try await source.connect()

        // Handshake: `convai` offered, init carries the overrides, graph at the negotiated rates.
        XCTAssertEqual(server.offeredSubprotocols.first, ["convai"])
        let initMsg = try XCTUnwrap(server.received.first)
        XCTAssertTrue(initMsg.contains(#""language":"tr""#), initMsg)
        XCTAssertEqual(device.rates?.input, 16000)
        XCTAssertEqual(device.rates?.output, 24000)
        XCTAssertEqual(states, [.initializing, .listening])

        // Mic after the graph is up.
        device.onCapture?([Float](repeating: 0.25, count: 1024))
        try await until { server.received.contains { $0.contains("user_audio_chunk") } }

        // Ping -> pong with the same event_id.
        let conn = try XCTUnwrap(server.connections.first)
        server.send(#"{"type":"ping","ping_event":{"event_id":42,"ping_ms":30}}"#, on: conn)
        try await until {
            server.received.contains { $0.contains(#""type":"pong""#) && $0.contains(#""event_id":42"#) }
        }

        // A turn: received -> thinking; played -> speaking with metrics; the server's
        // agent_response_complete + drained -> listening.
        server.send(audio(id: 1), on: conn)
        try await until { states.last == .thinking }
        XCTAssertEqual(device.scheduled.first?.frame, 2400)
        device.playedFrames = 2400 + 2400
        try await until { states.last == .speaking }
        try await until { (metrics.last?.level ?? 0) > 0.05 }
        server.send(#"{"type":"agent_response_complete","agent_response_complete_event":{"event_id":1}}"#, on: conn)
        device.playedFrames = 2400 + 4800
        try await until { states.last == .listening }

        // Interruption while audible: one barge-in, playback cleared, late chunk dropped.
        server.send(audio(id: 2), on: conn)
        try await until { device.scheduled.count == 2 }
        device.playedFrames = (device.scheduled.last?.frame ?? 0) + 100
        try await until { states.last == .speaking }
        server.send(#"{"type":"interruption","interruption_event":{"event_id":3}}"#, on: conn)
        try await until { interrupts == 1 && device.resets == 1 }
        server.send(audio(id: 2), on: conn)
        server.send(#"{"type":"vad_score","vad_score_event":{"vad_score":0.9}}"#, on: conn)
        try await until { server.sentCount == 8 }  // everything the server sent has been written
        try await Task.sleep(nanoseconds: 100_000_000)
        XCTAssertEqual(device.scheduled.count, 2, "the late chunk (event_id 2 < 3) was dropped")

        // The agent ends the conversation (close 1000): idle, no reconnect.
        server.close(conn)
        try await until { states.last == .idle }
        XCTAssertFalse(device.started)
        XCTAssertEqual(server.connections.count, 1)
        server.stop()
    }

    func testNonPcmInputFormatIsRejected() async throws {
        let server = try FakeWSServer(subprotocol: "convai")
        try await server.ready()
        server.onMessage = { conn, _ in
            server.send(
                #"{"type":"conversation_initiation_metadata","conversation_initiation_metadata_event":{"agent_output_audio_format":"pcm_16000","user_input_audio_format":"ulaw_8000"}}"#,
                on: conn)
        }
        let device = FakeDevice()
        let source = ElevenLabsVoiceSource(
            credential: "a", endpoint: URL(string: "ws://127.0.0.1:\(server.port)/")!,
            device: device, requestPermission: { true })
        var states: [AgentState] = []
        source.onStateChange { states.append($0) }
        do {
            try await source.connect()
            XCTFail("expected unsupportedInputFormat")
        } catch let e as ElevenLabsError {
            XCTAssertEqual(e, .unsupportedInputFormat("ulaw_8000"))
        }
        XCTAssertEqual(states.last, .idle)
        XCTAssertNil(device.rates, "the graph never started")
        server.stop()
    }

    func testBadAgentFailsWithoutAPromptOrTheMic() async throws {
        let server = try FakeWSServer(subprotocol: "convai")
        try await server.ready()
        server.onMessage = { conn, _ in conn.cancel() }  // unknown agent: closed before metadata
        let device = FakeDevice()
        var asked = 0
        let source = ElevenLabsVoiceSource(
            credential: "nope", endpoint: URL(string: "ws://127.0.0.1:\(server.port)/")!,
            device: device,
            requestPermission: {
                asked += 1
                return true
            })
        do {
            try await source.connect()
            XCTFail("expected closedDuringSetup")
        } catch let e as ElevenLabsError {
            if case .closedDuringSetup = e {} else { XCTFail("\(e)") }
        }
        XCTAssertEqual(asked, 0, "no permission prompt before the agent answered")
        XCTAssertFalse(device.started)
        server.stop()
    }

    func testPermissionDeniedAfterMetadataNeverStartsAudio() async throws {
        let server = try FakeWSServer(subprotocol: "convai")
        try await server.ready()
        server.onMessage = { conn, text in
            if text.contains("conversation_initiation_client_data") { server.send(self.metadata, on: conn) }
        }
        let device = FakeDevice()
        let source = ElevenLabsVoiceSource(
            credential: "a", endpoint: URL(string: "ws://127.0.0.1:\(server.port)/")!,
            device: device, requestPermission: { false })
        do {
            try await source.connect()
            XCTFail("expected permissionDenied")
        } catch let e as VoiceSourceError { XCTAssertEqual(e, .permissionDenied) }
        XCTAssertNil(device.rates)
        server.stop()
    }

    func testMissingCredential() async throws {
        do {
            try await ElevenLabsVoiceSource(credential: " ", device: FakeDevice(), requestPermission: { true })
                .connect()
            XCTFail("expected missingCredential")
        } catch let e as ElevenLabsError { XCTAssertEqual(e, .missingCredential) }
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
    var rates: (input: Int, output: Int)?
    var onCapture: (@Sendable ([Float]) -> Void)?
    var onClockReset: (() -> Void)?

    func start(inputRate: Int, outputRate: Int, onCapture: @escaping @Sendable ([Float]) -> Void) throws {
        started = true
        rates = (inputRate, outputRate)
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
    var subprotocols: [[String]] = []
}

/// A WebSocket server on 127.0.0.1 (random port): records the upgrade headers
/// (lowercased) and every text frame, and accepts one subprotocol.
@MainActor
final class FakeWSServer {
    private let listener: NWListener
    private let headerBox = HeaderBox()
    private(set) var port: UInt16 = 0
    var requestHeaders: [[String: String]] { headerBox.all }
    var offeredSubprotocols: [[String]] { headerBox.subprotocols }
    private(set) var received: [String] = []
    private(set) var connections: [NWConnection] = []
    private(set) var sentCount = 0
    var onMessage: ((NWConnection, String) -> Void)?

    init(subprotocol: String?) throws {
        let ws = NWProtocolWebSocket.Options()
        ws.autoReplyPing = true
        let box = headerBox
        ws.setClientRequestHandler(.main) { offered, headers in
            // Network.framework passes the offered subprotocols separately, not as a header.
            box.subprotocols.append(offered)
            box.all.append(
                Dictionary(headers.map { ($0.name.lowercased(), $0.value) }, uniquingKeysWith: { a, _ in a }))
            return .init(status: .accept, subprotocol: subprotocol)
        }
        let params = NWParameters.tcp
        params.defaultProtocolStack.applicationProtocols.insert(ws, at: 0)
        params.requiredLocalEndpoint = .hostPort(host: "127.0.0.1", port: .any)
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
            content: Data(text.utf8), contentContext: ctx, isComplete: true,
            completion: .contentProcessed { [weak self] _ in
                MainActor.assumeIsolated { self?.sentCount += 1 }
            })
    }

    /// A normal closure (1000), as when the agent ends the conversation.
    func close(_ conn: NWConnection) {
        let meta = NWProtocolWebSocket.Metadata(opcode: .close)
        meta.closeCode = .protocolCode(.normalClosure)
        let ctx = NWConnection.ContentContext(identifier: "close", metadata: [meta])
        conn.send(
            content: nil, contentContext: ctx, isComplete: true, completion: .contentProcessed { _ in conn.cancel() })
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
