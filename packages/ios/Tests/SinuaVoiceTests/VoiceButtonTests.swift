import SinuaVoice
import XCTest

/// The voice button's state machine against spec/voice-button-cases.json (the table Web
/// and Android read too), the controller on a fake source, and the SharedVoiceSource
/// fan-out it relies on. No audio.
@MainActor
final class VoiceButtonTests: XCTestCase {
    private func table() throws -> [[String: Any]] {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("spec/voice-button-cases.json")
        guard let data = try? Data(contentsOf: url) else {
            throw XCTSkip("spec/voice-button-cases.json not readable at \(url.path) (device run?)")
        }
        let root = try JSONSerialization.jsonObject(with: data) as! [String: Any]
        return root["cases"] as! [[String: Any]]
    }

    private func event(_ s: String) -> VoiceButtonEvent {
        let parts = s.split(separator: ":", maxSplits: 1).map(String.init)
        switch parts[0] {
        case "press": return .press
        case "release": return .release
        case "end": return .end
        case "connectOk": return .connectOk
        case "connectFail": return .connectFail(parts[1])
        case "dropped": return .dropped
        case "muted": return .muteChanged(parts[1] == "true")
        default: fatalError("unknown event \(s)")
        }
    }

    func testTheStateMachineFollowsTheSharedTable() throws {
        let cases = try table()
        XCTAssertGreaterThanOrEqual(cases.count, 10)
        for c in cases {
            let name = c["name"] as! String
            let mode = VoiceButtonMode(rawValue: c["mode"] as! String)!
            let canMute = c["canMute"] as! Bool
            var m = VoiceButtonModel()
            for step in c["steps"] as! [[Any]] {
                let ev = step[0] as! String
                let out = voiceButtonStep(m, event(ev), mode: mode, canMute: canMute)
                XCTAssertEqual(out.model.state.rawValue, step[1] as! String, "\(name): after \(ev)")
                XCTAssertEqual(out.effects.map(\.rawValue), step[2] as! [String], "\(name): effects of \(ev)")
                m = out.model
            }
        }
    }

    func testSharedSourceFansOutAndIsTheSameInstance() {
        let src = FakeButtonSource()
        let a = SharedVoiceSource.of(src)
        XCTAssertTrue(SharedVoiceSource.of(src) === a)
        XCTAssertTrue(SharedVoiceSource.of(a) === a)
        var one: [AgentState] = []
        var two: [AgentState] = []
        let off = a.listenState { one.append($0) }
        a.listenState { two.append($0) }
        src.stateCb?(.listening)
        off()
        src.stateCb?(.speaking)
        XCTAssertEqual(one, [.listening])
        XCTAssertEqual(two, [.listening, .speaking])
        XCTAssertEqual(a.state, .speaking)
    }

    func testTrackedViewsKeepTheirOwnTrackerAndFollowTheMute() {
        let src = FakeButtonSource()
        let shared = SharedVoiceSource.of(src)
        let a = shared.track()
        let b = shared.track()
        src.metricsCb?(VoiceMetrics(level: 0.5, bands: [0.4]))
        XCTAssertEqual(a.overrides.metrics.level, 0.5)
        XCTAssertEqual(b.overrides.metrics.level, 0.5)
        shared.setMuted(true)
        XCTAssertEqual(src.muted, [true])
        XCTAssertTrue(a.overrides.muted)
        b.release()
        src.metricsCb?(VoiceMetrics(level: 0.9, bands: [0.4]))
        XCTAssertEqual(b.overrides.metrics.level, 0.5, "released: no longer fed")
        XCTAssertEqual(a.overrides.metrics.level, 0.9)
    }

    func testControllerConnectsMutesAndFollowsARemoteDrop() async throws {
        let src = FakeButtonSource()
        let c = VoiceButtonController(source: src)
        var states: [VoiceButtonState] = []
        c.onChange { states.append($0.state) }
        c.press()
        XCTAssertEqual(c.state, .connecting)
        try await until { c.state == .listening }
        c.press()
        XCTAssertEqual(c.state, .muted)
        XCTAssertEqual(src.muted, [false, true], "connect unmutes first, then the press mutes")
        src.stateCb?(.idle)  // an agent idle while connected is not a drop
        XCTAssertEqual(c.state, .muted)
        src.connectionCb?(false)  // the agent hung up
        XCTAssertEqual(c.state, .ready)
        XCTAssertEqual(states, [.connecting, .listening, .muted, .ready])
    }

    func testControllerShowsAFailedConnect() async throws {
        let src = FakeButtonSource()
        src.fail = true
        let c = VoiceButtonController(source: src)
        c.press()
        try await until { c.state == .error }
        XCTAssertEqual(c.reason, "no credential")
    }

    func testAScreenReaderActivationInPushToTalkStartsLiveThenToggles() async throws {
        let src = FakeButtonSource()
        let c = VoiceButtonController(source: src, mode: .pushToTalk)
        c.assistiveActivate()
        try await until { c.state == .listening }
        c.assistiveActivate()
        XCTAssertEqual(c.state, .muted)
        c.assistiveActivate()
        XCTAssertEqual(c.state, .listening)
        XCTAssertEqual(c.mode, .pushToTalk)
    }

    private func until(_ timeout: TimeInterval = 3, _ cond: () -> Bool) async throws {
        let end = Date().addingTimeInterval(timeout)
        while !cond() {
            if Date() > end { throw TimedOut() }
            try await Task.sleep(nanoseconds: 5_000_000)
        }
    }

    struct TimedOut: Error {}
}

/// One callback of each kind, like every real source.
final class FakeButtonSource: VoiceSource {
    var metricsCb: ((VoiceMetrics) -> Void)?
    var stateCb: ((AgentState) -> Void)?
    var connectionCb: ((Bool) -> Void)?
    var muted: [Bool] = []
    var fail = false

    struct Failure: LocalizedError {
        var errorDescription: String? { "no credential" }
    }

    func connect() async throws {
        if fail { throw Failure() }
        await MainActor.run { connectionCb?(true) }
    }

    func disconnect() {
        connectionCb?(false)
        stateCb?(.idle)
    }

    func onMetrics(_ cb: @escaping (VoiceMetrics) -> Void) { metricsCb = cb }
    func onStateChange(_ cb: @escaping (AgentState) -> Void) { stateCb = cb }
    func onConnectionChange(_ cb: @escaping (Bool) -> Void) { connectionCb = cb }
    var reportsConnection: Bool { true }
    var supportsMute: Bool { true }
    func setMuted(_ m: Bool) { muted.append(m) }
}
