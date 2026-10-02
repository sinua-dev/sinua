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

    /// The catalog's `character` patterns: drawn with fills by default (docs/character.md).
    private static func characterPatternCount() throws -> Int {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("spec/parameters.json")
        let data = try Data(contentsOf: url)
        let root = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let objects = try XCTUnwrap(root["objects"] as? [[String: Any]])
        return objects.filter { $0["id"] as? String == "character" }
            .reduce(0) { $0 + (($1["patterns"] as? [[String: Any]])?.count ?? 0) }
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
        var perVertex = 0
        var filled = 0
        for s in states {
            guard let frame = frame(state: s, size: 64, t: 1.7) else {
                XCTFail("\(s) is in spec/parameters.json but does not render through CoreEngine")
                continue
            }
            // OldOrbPaint is the 2026-09-18 painter, from before per-vertex stroke colour
            // (golden 1.6.0, `Polyline.hues`): it can't draw a frame that uses it (no pattern
            // does by default since SinuaEdge went). That paint rule is checked across
            // platforms by MaterialsRenderTests instead.
            if frame.polylines.contains(where: { !$0.hues.isEmpty }) {
                perVertex += 1
                continue
            }
            // Nor fills (materials phase 1): a character is drawn with fills only
            // (docs/character.md). MaterialsRenderTests holds fills across platforms.
            if !frame.fills.isEmpty {
                filled += 1
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
        XCTAssertEqual(compared, (states.count - perVertex - filled) * 2, "one bitmap pair per pattern")
        XCTAssertEqual(perVertex, 0, "no pattern draws per-vertex colour by default")
        XCTAssertEqual(filled, try Self.characterPatternCount(), "exactly the character patterns draw fills by default")
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

    /// A pattern change cross-fades (the engine's `crossFade` technique), over the
    /// `crossFade` override here; then it settles on one frame.
    func testStatePlayerCrossFadesAPatternChangeThenSettles() throws {
        var p = FxStatePlayer()
        p.crossFade = 0.25
        let json =
            #"{"fxSpec":"1.8","object":"orb","pattern":"listening","states":{"speaking":{"pattern":"speaking"}}}"#
        p.setState(nil, spec: json)
        _ = p.frame(spec: json, elapsed: 1, dt: 0.016, inputs: [:], extra: [:])
        p.setState("speaking", spec: json)
        let mid = p.frame(spec: json, elapsed: 1, dt: 0.1, inputs: [:], extra: [:])
        XCTAssertNotNil(mid.previous)
        let r = { (s: String?) -> TransitionSide in
            let x = resolveFxSpecWith(json: json, state: s, inputs: [:], lowPower: false)
            return TransitionSide(
                state: x.state, speed: (resolvedOpts(state: x.state, size: x.size)?.speed ?? 1) * x.speed,
                overrides: x.overrides)
        }
        let want = try XCTUnwrap(
            transitionMix(from: r(nil), to: r("speaking"), size: 64, progress: 0.4, curve: "easeInOut"))
        XCTAssertEqual(want.technique, "crossFade")
        XCTAssertEqual(mid.blend, want.weight, accuracy: 1e-12)
        let end = p.frame(spec: json, elapsed: 1, dt: 0.2, inputs: [:], extra: [:])
        XCTAssertNil(end.previous)
        XCTAssertEqual(end.blend, 1)
    }

    /// The same pattern across states: one frame whose parameters flow (no dissolve outside
    /// the short count/choice swap window), landing exactly on the new state.
    func testStatePlayerInterpolatesASamePatternChange() throws {
        let json =
            #"{"fxSpec":"1.9","object":"orb","pattern":"glowing","states":{"idle":{"ink":0.6,"speed":0.5},"speaking":{"ink":1,"speed":1.2}},"transitions":{"default":{"duration":0.5,"curve":"linear"}}}"#
        XCTAssertEqual(
            fxSpecTransition(json: json, from: "idle", to: "speaking"), FxTransition(duration: 0.5, curve: "linear"))
        var p = FxStatePlayer()
        p.setState("idle", spec: json)
        _ = p.frame(spec: json, elapsed: 1, dt: 0.016, inputs: [:], extra: [:])
        let idleSpeed = p.speed(spec: json, inputs: [:])
        p.setState("speaking", spec: json)
        let mid = p.frame(spec: json, elapsed: 1, dt: 0.15, inputs: [:], extra: [:])
        XCTAssertNil(mid.previous, "one frame before the count/choice swap window: the parameters interpolate")
        let midSpeed = p.speed(spec: json, inputs: [:])
        XCTAssertGreaterThan(midSpeed, idleSpeed)
        let x = resolveFxSpecWith(json: json, state: "speaking", inputs: [:], lowPower: false)
        let preset = resolvedOpts(state: x.state, size: x.size)?.speed ?? 1
        XCTAssertLessThan(midSpeed, preset * x.speed)
        _ = p.frame(spec: json, elapsed: 1, dt: 0.4, inputs: [:], extra: [:])
        XCTAssertEqual(p.speed(spec: json, inputs: [:]), preset * x.speed, accuracy: 1e-12, "lands on speaking")
        let end = p.frame(spec: json, elapsed: 2, dt: 0.016, inputs: [:], extra: [:])
        XCTAssertEqual(
            end.frame, frameWithOverrides(state: x.state, size: x.size, t: 2 * preset * x.speed, overrides: x.overrides)
        )
    }

    /// `crossFade: 0` is a cut, as before.
    func testStatePlayerCutsWithCrossFadeZero() throws {
        let json =
            #"{"fxSpec":"1.8","object":"orb","pattern":"glowing","states":{"idle":{"ink":0.6},"speaking":{"ink":1}}}"#
        var p = FxStatePlayer()
        p.crossFade = 0
        p.setState("idle", spec: json)
        _ = p.frame(spec: json, elapsed: 1, dt: 0.016, inputs: [:], extra: [:])
        p.setState("speaking", spec: json)
        let f = p.frame(spec: json, elapsed: 1, dt: 0.016, inputs: [:], extra: [:])
        XCTAssertNil(f.previous)
        XCTAssertEqual(
            f.frame, FxStatePlayer.render(spec: json, state: "speaking", elapsed: 1, inputs: [:], extra: [:]))
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

    /// A named palette's dark variant (design note 23): the view passes `dark` in a dark
    /// theme and the engine picks it. Cuppa in ocean: the sleeve is #D4EDF7 light, #72ADCA dark.
    func testADarkThemePicksThePalettesDarkVariant() throws {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("spec/examples/themed-cuppa.fxspec.json")
        guard let json = try? String(contentsOf: url, encoding: .utf8) else { throw XCTSkip("spec not readable") }
        func sleeve(_ theme: FxTheme) throws -> Double {
            let r = ImageRenderer(
                content: SinuaView(spec: json, theme: theme, reducedMotion: .always).frame(width: 160, height: 160))
            r.scale = 2
            let cg = try XCTUnwrap(r.cgImage)
            let bytes = CFDataGetBytePtr(cg.dataProvider!.data!)!
            // The sleeve band (box y ~140, left of the heart), averaged over a small patch.
            var sum = 0.0
            var n = 0.0
            for y in 222..<228 {
                for x in 92..<98 {
                    let i = y * cg.bytesPerRow + x * 4
                    sum += (Double(bytes[i]) + Double(bytes[i + 1]) + Double(bytes[i + 2])) / (3 * 255)
                    n += 1
                }
            }
            return sum / n
        }
        let light = try sleeve(.light)
        let dark = try sleeve(.dark)
        XCTAssertGreaterThan(light - dark, 0.12, "light \(light), dark \(dark)")
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
