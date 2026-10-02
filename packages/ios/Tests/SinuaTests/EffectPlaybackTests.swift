import XCTest

@testable import Sinua

/// A view model plays a one-shot effect (docs/fx-view.md, *One-shot effects*): each new
/// trigger once, its keys for its duration, its words spoken at once. No audio.
@MainActor
final class EffectPlaybackTests: XCTestCase {
    private func config(
        _ effect: SinuaEffectTrigger?, labels: [String: String] = [:], announce: Bool? = nil
    ) -> FxConfig {
        FxConfig(
            input: .state("completing", 64, [:], 1), voice: nil, voiceOverrides: nil, specState: nil, inputs: [:],
            voiceLevelInput: nil, crossFade: nil, theme: .light, paused: false, reducedMotion: .never, label: "Goal",
            maxFps: nil, lowPower: .auto, onFrame: nil, labels: labels, announce: announce, effect: effect)
    }

    func testEachTriggerPlaysOnceForItsDurationAndIsSpoken() {
        var now = 100.0
        var said: [String] = []
        FxModel.now = { now }
        FxModel.postAnnouncement = { said.append($0) }
        defer {
            FxModel.now = { ProcessInfo.processInfo.systemUptime }
            FxModel.postAnnouncement = { _ in }
        }
        let m = FxModel()
        let success = SinuaEffectTrigger(.success)
        m.configureIfNeeded(config(success))
        XCTAssertEqual(said, ["Done"])
        XCTAssertEqual(m.effectKeys(reduced: false)["effectCode"], 1)
        now += 0.5
        XCTAssertEqual(m.effectKeys(reduced: true)["effectAge"]!, 0.5, accuracy: 1e-9)
        XCTAssertEqual(m.effectKeys(reduced: true)["effectReduced"], 1)
        m.configureIfNeeded(config(success))  // the same value again: not replayed
        XCTAssertEqual(said, ["Done"])
        now += 0.5
        XCTAssertEqual(m.effectKeys(reduced: false), [:], "ended after 0.9 s")
        m.configureIfNeeded(config(SinuaEffectTrigger(.celebrate), labels: ["effect:celebrate": "Hedef tamam"]))
        XCTAssertEqual(said, ["Done", "Hedef tamam"])
        XCTAssertEqual(m.effectKeys(reduced: false)["effectCode"], 3)
        m.configureIfNeeded(config(SinuaEffectTrigger(.error), announce: false))
        XCTAssertEqual(said, ["Done", "Hedef tamam"], "announce: false keeps quiet")
        XCTAssertEqual(m.effectKeys(reduced: false)["effectCode"], 2)
    }

    func testTheTapHopIsSilentCarriesTheTapAndNeverCutsAnotherEffect() {
        var now = 100.0
        var said: [String] = []
        FxModel.now = { now }
        FxModel.postAnnouncement = { said.append($0) }
        defer {
            FxModel.now = { ProcessInfo.processInfo.systemUptime }
            FxModel.postAnnouncement = { _ in }
        }
        let m = FxModel()
        m.configureIfNeeded(config(nil))
        m.hop(at: (0.5, -0.25))
        XCTAssertEqual(said, [], "the hop is silent")
        XCTAssertEqual(m.effectKeys(reduced: false)["effectCode"], 4)
        XCTAssertEqual(m.effectKeys(reduced: false)["tapX"], 0.5)
        XCTAssertEqual(m.effectKeys(reduced: false)["tapY"], -0.25)
        now += 0.2
        m.hop(at: (1, 1))  // within 0.5 s of the last one: ignored
        XCTAssertEqual(m.effectKeys(reduced: false)["tapX"], 0.5)
        now += 0.5
        XCTAssertEqual(m.effectKeys(reduced: false), [:], "ended after 0.6 s")
        m.configureIfNeeded(config(SinuaEffectTrigger(.success)))
        m.hop(at: (0, 0))
        XCTAssertEqual(m.effectKeys(reduced: false)["effectCode"], 1, "a tap doesn't cut success")
        m.configureIfNeeded(config(SinuaEffectTrigger(.hop)))
        XCTAssertEqual(m.effectKeys(reduced: false)["effectCode"], 1, "nor does a .hop trigger")
        now += 1
        m.configureIfNeeded(config(SinuaEffectTrigger(.hop)))
        XCTAssertEqual(m.effectKeys(reduced: false)["effectCode"], 4)
        XCTAssertNil(m.effectKeys(reduced: false)["tapX"], "a triggered hop has no tap")
    }

    func testTheExpressionEasesInOverHalfASecondAndNilAddsNothing() {
        var now = 100.0
        FxModel.now = { now }
        defer { FxModel.now = { ProcessInfo.processInfo.systemUptime } }
        let m = FxModel()
        XCTAssertEqual(m.expressionKeys(nil, reduced: false), [:], "unset: the spec decides")
        _ = m.expressionKeys("happy", reduced: false)
        now += 0.3
        XCTAssertEqual(m.expressionKeys("happy", reduced: false)["expressionHappy"]!, 0.5, accuracy: 1e-9)
        now += 0.3
        XCTAssertEqual(m.expressionKeys("happy", reduced: false)["expressionHappy"], 1)
        XCTAssertEqual(m.expressionKeys("sad", reduced: true)["expressionSad"], 1, "reduced motion cuts")
        XCTAssertEqual(m.expressionKeys("sad", reduced: true)["expressionHappy"], 0)
    }

    func testThePaletteResolvesThroughTheEngineAndFollowsThePattern() {
        let m = FxModel()
        XCTAssertEqual(m.paletteKeys(pattern: "buzzy", [:]), [:], "empty: the character's own")
        let red = m.paletteKeys(pattern: "buzzy", ["shell": "#E63946"])
        XCTAssertEqual(red["palette.shell.w"], 1)
        XCTAssertNotNil(red["palette.shellDark.l"], "the tones follow")
        let bean = m.paletteKeys(pattern: "bean", ["bean": "#2B1A12"])
        XCTAssertGreaterThan(bean["palette.ink.l"] ?? 0, 0.8, "a dark ground lifts the ink")
        XCTAssertEqual(m.paletteKeys(pattern: "bean", ["beam": "#000000"]), [:], "an unknown slot adds nothing")
    }
}
