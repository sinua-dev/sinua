import CoreEngine
import SinuaVoice
import SwiftUI
import XCTest

@testable import Sinua

/// FxPaint must draw exactly what the Studio's OrbPaint drew before the
/// move, and SinuaView's clock/spec math must equal the engine's own helpers.
@MainActor
final class SinuaViewTests: XCTestCase {
    /// Every pattern the engine publishes, read from `spec/parameters.json` --
    /// the catalog `catalog.rs` pins to the engine's presets. This was a
    /// hand-written 34-name string, byte-identical to one in SinuaViewTest.kt,
    /// so a 35th pattern went unrendered here while the suite still passed.
    /// Simulator runs read the host's
    /// file; anywhere it can't be read, the test skips with a message.
    private static func catalogPatterns() throws -> [String] {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("spec/parameters.json")
        guard let data = try? Data(contentsOf: url) else {
            throw XCTSkip("spec/parameters.json not readable at \(url.path) (device run?)")
        }
        let root = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let objects = try XCTUnwrap(root["objects"] as? [[String: Any]])
        return objects.flatMap { ($0["patterns"] as? [[String: Any]] ?? []).compactMap { $0["id"] as? String } }
    }

    private func bitmap(_ draw: @escaping (inout GraphicsContext, CGSize) -> Void) -> Data {
        let view = Canvas { ctx, size in draw(&ctx, size) }.frame(width: 128, height: 128)
        let r = ImageRenderer(content: view)
        r.scale = 2
        guard let cg = r.cgImage, let provider = cg.dataProvider, let data = provider.data else { return Data() }
        return data as Data
    }

    func testFxPaintIsPixelIdenticalToTheStudioPainter() throws {
        let states = try Self.catalogPatterns()
        // A floor, not a count: it only guards a loader that read too little.
        XCTAssertGreaterThanOrEqual(
            states.count, 34, "only \(states.count) patterns read -- the file or the loader moved")
        var compared = 0
        for s in states {
            guard let frame = frame(state: s, size: 64, t: 1.7) else {
                XCTFail("\(s) is in spec/parameters.json but does not render through CoreEngine")
                continue
            }
            for dark in [false, true] {
                let old = bitmap { ctx, size in
                    OldOrbPaint.draw(frame, into: &ctx, size: size, engineSize: 64, dark: dark)
                }
                let new = bitmap { ctx, size in FxPaint.draw(frame, into: &ctx, size: size, engineSize: 64, dark: dark)
                }
                XCTAssertFalse(old.isEmpty)
                XCTAssertEqual(old, new, "\(s) dark=\(dark)")
                compared += 1
            }
        }
        // The denominator, exactly: two themes for every pattern that exists.
        XCTAssertEqual(compared, states.count * 2, "one bitmap pair per pattern")
        print("FxPaint parity: \(compared) bitmaps identical across \(states.count) patterns")
    }

    func testSpecPathEqualsFrameFromFxSpec() throws {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("spec/examples")
        guard let files = try? FileManager.default.contentsOfDirectory(atPath: url.path) else {
            throw XCTSkip("spec/examples not readable")
        }
        var checked = 0
        for f in files where f.hasSuffix(".fxspec.json") {
            let json = try String(contentsOf: url.appendingPathComponent(f), encoding: .utf8)
            guard let want = frameFromFxSpec(json: json, elapsed: 1.3) else { continue }
            XCTAssertEqual(FxStatePlayer.render(spec: json, state: nil, elapsed: 1.3, inputs: [:], extra: [:]), want, f)
            checked += 1
        }
        XCTAssertGreaterThan(checked, 3)
    }

    func testStatePlayerCrossFadesThenSettles() throws {
        var p = FxStatePlayer()
        p.crossFade = 0.25
        let json =
            #"{"fxSpec":"1.8","object":"orb","pattern":"listening","states":{"speaking":{"pattern":"speaking"}}}"#
        p.setState(nil)
        _ = p.frame(spec: json, elapsed: 1, dt: 0.016, inputs: [:], extra: [:])
        p.setState("speaking")
        let mid = p.frame(spec: json, elapsed: 1, dt: 0.1, inputs: [:], extra: [:])
        XCTAssertNotNil(mid.previous)
        XCTAssertEqual(mid.blend, 1 - pow(1 - 0.4, 3), accuracy: 1e-12)
        let end = p.frame(spec: json, elapsed: 1, dt: 0.2, inputs: [:], extra: [:])
        XCTAssertNil(end.previous)
        XCTAssertEqual(end.blend, 1)
    }

    func testSinuaViewRendersASpecAndAState() throws {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("spec/examples/voice-assistant.fxspec.json")
        guard let json = try? String(contentsOf: url, encoding: .utf8) else { throw XCTSkip("spec not readable") }
        for (name, view) in [
            ("spec", AnyView(SinuaView(spec: json))), ("state", AnyView(SinuaView(pattern: "speaking"))),
            ("dark", AnyView(SinuaView(pattern: "listening", theme: .dark))),
        ] {
            let r = ImageRenderer(content: view.frame(width: 160, height: 160))
            r.scale = 2
            let img = try XCTUnwrap(r.uiImage, name)
            let data = try XCTUnwrap(img.pngData())
            let attachment = XCTAttachment(data: data, uniformTypeIdentifier: "public.png")
            attachment.name = "SinuaView-\(name).png"
            attachment.lifetime = .keepAlways
            add(attachment)
            // Non-blank: some pixel has alpha.
            let cg = try XCTUnwrap(img.cgImage)
            let bytes = CFDataGetBytePtr(cg.dataProvider!.data!)!
            let count = cg.bytesPerRow * cg.height
            var inked = 0
            for i in stride(from: 3, to: count, by: 4) where bytes[i] > 0 { inked += 1 }
            XCTAssertGreaterThan(inked, 50, "\(name) drew something")
            try data.write(
                to: URL(fileURLWithPath: NSTemporaryDirectory()).appendingPathComponent("SinuaView-\(name).png"))
            print("SinuaView render \(name): \(inked) inked pixels -> \(NSTemporaryDirectory())SinuaView-\(name).png")
        }
    }

    func testFamilyVoiceDefaultsMatchTheStudios() {
        XCTAssertEqual(FxModel.voiceOptions(family: "orb", overrides: [:]).bandEaseRate, .infinity)
        let sig = FxModel.voiceOptions(family: "signal", overrides: ["historyCount": 24])
        XCTAssertEqual(sig.audioStrength, 0)
        XCTAssertEqual(sig.historyCount, 24)
        XCTAssertNil(FxModel.voiceOptions(family: "ring", overrides: [:]).historyCount)
    }
}

@MainActor
final class PerformanceTests: XCTestCase {
    private func drawsPerSecond(_ maxFps: Double?, hz: Double, seconds: Double = 10, jitter: Double = 0) -> Double {
        var p = FramePacer(maxFps: maxFps)
        var seed: UInt64 = 1
        var draws = 0
        let frames = Int(hz * seconds)
        for i in 0..<frames {
            seed = seed &* 6_364_136_223_846_793_005 &+ 1
            let r = (Double(seed >> 33) / Double(1 << 31)) - 0.5
            if p.shouldDraw(atMs: 1000 + Double(i) * 1000 / hz + r * 2 * jitter) { draws += 1 }
        }
        return Double(draws) / seconds
    }

    func testPacerIsExactWithoutDrift() {
        XCTAssertEqual(drawsPerSecond(30, hz: 60), 30, accuracy: 0.1)
        XCTAssertEqual(drawsPerSecond(30, hz: 120), 30, accuracy: 0.1)
        XCTAssertEqual(drawsPerSecond(24, hz: 60), 24, accuracy: 0.2)
        XCTAssertEqual(drawsPerSecond(30, hz: 60, jitter: 0.8), 30, accuracy: 0.3)
        XCTAssertEqual(drawsPerSecond(nil, hz: 60), 60)
        XCTAssertEqual(drawsPerSecond(0, hz: 60), 60)
        XCTAssertEqual(drawsPerSecond(120, hz: 60), 60)
    }

    func testPerformancePolicy() {
        XCTAssertEqual(fxPerformance(lowPower: false, optionMaxFps: nil), FxPerformance(maxFps: nil, overrides: [:]))
        XCTAssertEqual(
            fxPerformance(lowPower: true, optionMaxFps: nil),
            FxPerformance(maxFps: 30, overrides: ["glowStrength": 0, "particleStrength": 0]))
        XCTAssertEqual(
            fxPerformance(lowPower: true, optionMaxFps: nil, specMaxFps: 20, specHandlesLowPower: true),
            FxPerformance(maxFps: 20, overrides: [:]))
        XCTAssertEqual(
            fxPerformance(lowPower: false, optionMaxFps: 24, specMaxFps: 60), FxPerformance(maxFps: 24, overrides: [:]))
        XCTAssertEqual(
            fxPerformance(lowPower: true, optionMaxFps: nil, specMaxFps: 24),
            FxPerformance(maxFps: 24, overrides: ["glowStrength": 0, "particleStrength": 0]))
    }

    func testSpec12LowPowerCapsAndSheds() {
        let spec =
            #"{"fxSpec":"1.8","object":"orb","pattern":"composing","materials":{"glow":{"strength":1}},"performance":{"maxFps":50,"lowPower":{"maxFps":20,"disable":["glow"]}}}"#
        let m = FxModel()
        let c = FxConfig(
            input: .spec(spec), voice: nil, voiceOverrides: nil, specState: nil, inputs: [:], voiceLevelInput: nil,
            crossFade: 0, theme: .light, paused: false, reducedMotion: .never, label: nil, maxFps: nil, lowPower: .auto,
            onFrame: nil)
        XCTAssertEqual(m.performance(c, lowPower: false).maxFps, 50)
        XCTAssertEqual(m.performance(c, lowPower: true), FxPerformance(maxFps: 20, overrides: [:]))
        let shed = FxStatePlayer.render(spec: spec, state: nil, elapsed: 1, inputs: [:], extra: [:], lowPower: true)
        let full = FxStatePlayer.render(spec: spec, state: nil, elapsed: 1, inputs: [:], extra: [:], lowPower: false)
        XCTAssertNotNil(shed)
        XCTAssertNotEqual(shed, full, "glow shed under low power")
        XCTAssertEqual(
            resolveFxSpecWith(json: spec, state: nil, inputs: [:], lowPower: true).disabledMaterials, ["glow"])
    }

    func testLowPowerMonitorFollowsTheNotification() async {
        let center = NotificationCenter()
        let flag = Flag()
        let m = LowPowerMonitor(center: center, read: { flag.value })
        XCTAssertFalse(m.isLowPowerModeEnabled)
        flag.value = true
        center.post(name: .NSProcessInfoPowerStateDidChange, object: nil)
        for _ in 0..<50 where !m.isLowPowerModeEnabled { try? await Task.sleep(nanoseconds: 10_000_000) }
        XCTAssertTrue(m.isLowPowerModeEnabled)
    }

    func testSinuaViewReportsFrameStats() throws {
        let box = StatsBox()
        let view = SinuaView(pattern: "composing", lowPower: .on, onFrame: { box.stats.append($0) }).frame(
            width: 120, height: 120)
        let r = ImageRenderer(content: view)
        _ = r.uiImage
        XCTAssertFalse(box.stats.isEmpty, "onFrame fired")
        XCTAssertGreaterThanOrEqual(box.stats[0].computeMs, 0)
    }
}

final class Flag: @unchecked Sendable { var value = false }
final class StatsBox: @unchecked Sendable { var stats: [FxFrameStats] = [] }
