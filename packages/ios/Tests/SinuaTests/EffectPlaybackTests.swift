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
}
