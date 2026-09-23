import CoreEngine
import SwiftUI
import XCTest

@testable import Sinua

/// The generated typed components (Sources/Sinua/Generated, scripts/codegen):
/// props -> engine overrides, pattern coverage, and that they draw what SinuaView draws.
@MainActor
final class TypedComponentsTests: XCTestCase {
    /// RGBA bytes of `view` drawn over opaque white. ImageRenderer can hand back stale pixels in
    /// transparent areas once other renders ran in the process (a bare `Color.clear` read 20091
    /// inked pixels mid-suite), so everything is composited on white and "ink" means non-white.
    private func pixels<V: View>(_ view: V) throws -> Data {
        let r = ImageRenderer(
            content: ZStack {
                Color.white
                view
            }.frame(width: 128, height: 128))
        r.scale = 2
        let cg = try XCTUnwrap(r.cgImage)
        let ctx = try XCTUnwrap(
            CGContext(
                data: nil, width: cg.width, height: cg.height, bitsPerComponent: 8, bytesPerRow: cg.width * 4,
                space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ctx.draw(cg, in: CGRect(x: 0, y: 0, width: cg.width, height: cg.height))
        let base = try XCTUnwrap(ctx.data)
        return Data(bytes: base, count: ctx.bytesPerRow * cg.height)
    }

    private func inked(_ data: Data) -> Int {
        var n = 0
        for i in stride(from: 0, to: data.count, by: 4) where data[i] < 250 || data[i + 1] < 250 || data[i + 2] < 250 {
            n += 1
        }
        return n
    }

    func testOverridesMapPropsToEngineKeys() {
        XCTAssertEqual(
            SinuaRing(pattern: .completing, progress: 0.4, strokeWidth: 0.1).overrides(),
            ["progress": 0.4, "strokeWidth": 0.1])
        XCTAssertEqual(
            SinuaRing(pattern: .tracking, progress: [0.2, 0.5], ringCount: 2).overrides(),
            ["progress0": 0.2, "progress1": 0.5, "ringCount": 2])
        XCTAssertEqual(SinuaRing(pattern: .tracking, progress: 0.7).overrides(), ["progress0": 0.7])
        XCTAssertEqual(SinuaRing(pattern: .completing, progress: [0.3, 0.9]).overrides(), ["progress": 0.3])
        XCTAssertEqual(SinuaRing(pattern: .measuring, fill: true, marker: false).overrides(), ["fill": 1, "marker": 0])
        XCTAssertEqual(SinuaRing(pattern: .stepping, segment: [1, 0.5]).overrides(), ["segment0": 1, "segment1": 0.5])
        XCTAssertEqual(
            SinuaRing(pattern: .loading, glow: .init(blend: .additive, strength: 0.6)).overrides(),
            ["glowStrength": 0.6, "glowBlend": 1])
        XCTAssertEqual(
            SinuaOrb(pattern: .working, orbitParticles: 4, particles: .init(count: 12, style: .orbit)).overrides(),
            ["particles": 4, "particleCount": 12, "particleStyle": 2])
        XCTAssertEqual(SinuaSignal(pattern: .waveform).overrides(), [:])
    }

    func testEveryEnginePatternHasACase() {
        let cases =
            SinuaOrb.Pattern.allCases.map(\.rawValue) + SinuaSignal.Pattern.allCases.map(\.rawValue)
            + SinuaRing.Pattern.allCases.map(\.rawValue) + SinuaCore.Pattern.allCases.map(\.rawValue)
            + SinuaBeacon.Pattern.allCases.map(\.rawValue)
        XCTAssertEqual(Set(cases).count, cases.count)
        for p in cases { XCTAssertNotNil(frame(state: p, size: 64, t: 1), p) }
    }

    func testDrawsExactlyWhatSinuaViewDraws() throws {
        let typed = try pixels(SinuaRing(pattern: .tracking, progress: [0.2, 0.9], ringCount: 2, paused: true))
        let plain = try pixels(
            SinuaView(
                pattern: "tracking", overrides: ["progress0": 0.2, "progress1": 0.9, "ringCount": 2], paused: true))
        XCTAssertGreaterThan(inked(typed), 50)
        XCTAssertEqual(typed, plain)
        let other = try pixels(SinuaRing(pattern: .tracking, progress: [0.9, 0.2], ringCount: 2, paused: true))
        XCTAssertNotEqual(typed, other, "the props reach the engine")
    }

    func testSpecForAnotherObjectDrawsNothingAndReportsIt() throws {
        let orbSpec = #"{"fxSpec":"1.8","object":"orb","pattern":"glowing"}"#
        let ring = SinuaRing(spec: orbSpec, onError: { _ in })
        XCTAssertEqual(ring.specError, SinuaSpecError(expected: "ring", found: "orb"))
        XCTAssertEqual(inked(try pixels(Color.clear)), 0, "control: the readback itself")
        XCTAssertEqual(inked(try pixels(ring)), 0)
        XCTAssertNil(SinuaOrb(spec: orbSpec).specError)
        XCTAssertNil(SinuaRing(spec: "not json").specError, "unreadable specs are SinuaView's to report")
    }

    func testSpecPathPlaysTheSpec() throws {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("spec/examples/voice-assistant.fxspec.json")
        guard let json = try? String(contentsOf: url, encoding: .utf8) else { throw XCTSkip("spec not readable") }
        guard let object = sinuaSpecObject(json) else { return XCTFail("example has an object") }
        let view: AnyView =
            switch object {
            case "orb": AnyView(SinuaOrb(spec: json, paused: true))
            case "signal": AnyView(SinuaSignal(spec: json, paused: true))
            case "ring": AnyView(SinuaRing(spec: json, paused: true))
            case "core": AnyView(SinuaCore(spec: json, paused: true))
            default: AnyView(SinuaBeacon(spec: json, paused: true))
            }
        XCTAssertEqual(try pixels(view), try pixels(SinuaView(spec: json, paused: true)))
    }

    /// FX Spec 1.7 labels: the deprecated SinuaView inits draw exactly what the new ones draw.
    @available(*, deprecated)
    func testDeprecatedLabelsDrawTheSame() throws {
        let overrides = ["glowStrength": 0.5]
        XCTAssertEqual(
            try pixels(SinuaView(pattern: "breathing", overrides: overrides, paused: true)),
            try pixels(SinuaView(state: "breathing", overrides: overrides, paused: true)))
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("spec/examples/voice-assistant.fxspec.json")
        guard let json = try? String(contentsOf: url, encoding: .utf8) else { throw XCTSkip("spec not readable") }
        let new = try pixels(SinuaView(spec: json, state: "speaking", paused: true))
        XCTAssertEqual(new, try pixels(SinuaView(spec: json, specState: "speaking", paused: true)))
        XCTAssertGreaterThan(inked(new), 50)
    }
}
