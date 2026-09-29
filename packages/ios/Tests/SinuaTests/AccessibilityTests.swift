import CoreEngine
import SinuaVoice
import XCTest

@testable import Sinua

/// State-aware accessibility (docs/fx-view.md, *Accessibility*): the engine's announcer
/// against spec/a11y-announce-vectors.json (the table Web and Android read too), and the
/// view model's name, announcements, haptic and the 1.9 rules glue. No audio.
@MainActor
final class AccessibilityTests: XCTestCase {
    private func vectors() throws -> [String: Any] {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("spec/a11y-announce-vectors.json")
        guard let data = try? Data(contentsOf: url) else {
            throw XCTSkip("spec/a11y-announce-vectors.json not readable at \(url.path) (device run?)")
        }
        return try JSONSerialization.jsonObject(with: data) as! [String: Any]
    }

    /// The host loop every platform runs: a step on each change, and at `recheckAt`.
    private func run(_ steps: [(Double, String?)], end: Double) -> [(Double, String)] {
        var s = AnnouncerState(started: false, current: nil, since: 0, last: nil, lastAt: 0)
        var said: [(Double, String)] = []
        var pending: Double?
        var i = 0
        var words: String?
        while true {
            let next = i < steps.count ? steps[i].0 : nil
            guard let t = [next, pending].compactMap({ $0 }).min(), t <= end else { break }
            if next == t {
                words = steps[i].1
                i += 1
            }
            let out = a11yAnnounceStep(prev: s, words: words, now: t)
            s = out.state
            pending = out.recheckAt
            if let w = out.announce { said.append((t, w)) }
        }
        return said
    }

    func testTheAnnouncerMatchesTheSharedVectors() throws {
        let v = try vectors()
        let cases = v["cases"] as! [[String: Any]]
        XCTAssertGreaterThanOrEqual(cases.count, 6)
        for c in cases {
            let steps = (c["steps"] as! [[Any]]).map { ($0[0] as! Double, $0[1] as? String) }
            let want = (c["said"] as! [[Any]]).map { "\($0[0] as! Double) \($0[1] as! String)" }
            let got = run(steps, end: c["end"] as! Double).map { "\($0.0) \($0.1)" }
            XCTAssertEqual(got, want, c["name"] as! String)
        }
        for n in v["names"] as! [[String: Any]] {
            XCTAssertEqual(
                a11yAccessibleName(
                    name: n["name"] as! String, state: n["state"] as? String,
                    specWords: n["spec"] as! [String: String], appWords: n["app"] as! [String: String]),
                n["expect"] as! String)
        }
    }

    private func config(
        _ input: FxInput, voice: VoiceSource? = nil, inputs: [String: Double] = [:],
        labels: [String: String] = [:], announce: Bool? = nil, haptics: Bool = false, rules: Bool = true
    ) -> FxConfig {
        FxConfig(
            input: input, voice: voice, voiceOverrides: nil, specState: nil, inputs: inputs, voiceLevelInput: nil,
            crossFade: nil, theme: .light, paused: false, reducedMotion: .never, label: nil, maxFps: nil,
            lowPower: .auto, onFrame: nil, labels: labels, announce: announce, haptics: haptics, rules: rules)
    }

    /// Lets the model's deferred label update and any due recheck run.
    private func settle(_ seconds: Double = 0.05) async {
        try? await Task.sleep(nanoseconds: UInt64(seconds * 1e9))
    }

    func testTheNameFollowsTheVoiceAndAHeldChangeIsSpokenOnce() async {
        var said: [String] = []
        var taps = 0
        FxModel.postAnnouncement = { said.append($0) }
        FxModel.playHaptic = { taps += 1 }
        defer {
            FxModel.postAnnouncement = { _ in }
            FxModel.playHaptic = {}
        }
        let src = FakeStateSource()
        let m = FxModel()
        m.configureIfNeeded(config(.state("glowing", 64, [:], 1), voice: src, haptics: true))
        await settle()
        XCTAssertEqual(m.a11yLabel, "glowing")
        src.stateCb?(.listening)
        await settle()
        XCTAssertEqual(m.a11yLabel, "glowing, listening")
        XCTAssertEqual(taps, 1, "a light tap when the agent starts listening")
        XCTAssertEqual(said, [], "not before the 1 s hold")
        await settle(1.2)
        XCTAssertEqual(said, ["glowing, listening"])
        // A flip under a second says nothing more.
        src.stateCb?(.speaking)
        await settle(0.3)
        src.stateCb?(.listening)
        await settle(1.3)
        XCTAssertEqual(said, ["glowing, listening"])
    }

    func testLabelsWinAndAnnounceFalseKeepsQuiet() async {
        var said: [String] = []
        FxModel.postAnnouncement = { said.append($0) }
        defer { FxModel.postAnnouncement = { _ in } }
        let spec =
            #"{"fxSpec":"1.9","object":"orb","pattern":"glowing","name":"Coach","states":{"listening":{},"speaking":{}},"accessibility":{"states":{"listening":"Coach is listening"}}}"#
        let src = FakeStateSource()
        let m = FxModel()
        m.configureIfNeeded(config(.spec(spec), voice: src, labels: ["speaking": "Koç konuşuyor"], announce: false))
        src.stateCb?(.listening)
        await settle()
        XCTAssertEqual(m.a11yLabel, "Coach is listening")
        src.stateCb?(.speaking)
        await settle(1.2)
        XCTAssertEqual(m.a11yLabel, "Koç konuşuyor")
        XCTAssertEqual(said, [], "announce: false")
    }

    func testRulesDeriveTheStateWithHysteresisUnlessAVoiceIsBound() async {
        let spec =
            #"{"fxSpec":"1.9","object":"orb","pattern":"glowing","name":"Heart","states":{"intense":{}},"rules":[{"when":{"input":"hr","gt":150},"state":"intense","hysteresis":5}],"accessibility":{"states":{"intense":"Heart rate high"}}}"#
        let m = FxModel()
        func label(_ hr: Double, rules: Bool = true) async -> String {
            m.configureIfNeeded(config(.spec(spec), inputs: ["hr": hr], rules: rules))
            await settle()
            return m.a11yLabel
        }
        let a = await label(140)
        let b = await label(151)
        let c = await label(148)
        let d = await label(145)
        let e = await label(160, rules: false)
        XCTAssertEqual([a, b, c, d, e], ["Heart", "Heart rate high", "Heart rate high", "Heart", "Heart"])
    }
}

/// One callback of each kind, like every real source.
final class FakeStateSource: VoiceSource {
    var stateCb: ((AgentState) -> Void)?
    func connect() async throws {}
    func disconnect() {}
    func onMetrics(_ cb: @escaping (VoiceMetrics) -> Void) {}
    func onStateChange(_ cb: @escaping (AgentState) -> Void) { stateCb = cb }
}
