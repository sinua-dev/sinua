import Foundation
import XCTest

@testable import CoreEngine

/// FX Spec through the UniFFI boundary on Apple's runtime: every
/// `spec/examples/*.fxspec.json` resolves with no errors, and
/// `frameFromFxSpec` renders exactly what `frameWithOverrides(resolved)` at
/// `elapsed * presetSpeed * spec.speed` does -- the property
/// `crates/core_engine/src/fx_spec.rs` checks natively and
/// `packages/core/test/fx-spec.test.mjs` checks on wasm. Read via
/// `#filePath` like `SinuaGoldenTests`, skipping where the repo isn't
/// visible (device runs).
final class FxSpecTests: XCTestCase {
    private func examplesDir() -> URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent()  // CoreEngineTests
            .deletingLastPathComponent()  // Tests
            .deletingLastPathComponent()  // ios
            .deletingLastPathComponent()  // packages
            .deletingLastPathComponent()  // repo root
            .appendingPathComponent("spec/examples")
    }

    func testExamplesRenderLikeOverrides() throws {
        let dir = examplesDir()
        guard let files = try? FileManager.default.contentsOfDirectory(atPath: dir.path) else {
            throw XCTSkip("spec/examples not readable at \(dir.path) (device run?)")
        }
        let specs = files.filter { $0.hasSuffix(".fxspec.json") }.sorted()
        XCTAssertGreaterThanOrEqual(specs.count, 8)
        for file in specs {
            let json = try String(contentsOf: dir.appendingPathComponent(file), encoding: .utf8)
            // v1.1: every `states` key too, with the fixed inputs every
            // platform's parity test passes (fx-spec.test.mjs's TEST_INPUTS).
            let keys: [String?] = [nil] + CoreEngine.resolveFxSpec(json: json).stateKeys
            for key in keys {
                for low in [false, true] {
                    let label = "\(file) \(key ?? "<base>") lowPower=\(low)"
                    let r = CoreEngine.resolveFxSpecWith(json: json, state: key, inputs: Self.inputs, lowPower: low)
                    XCTAssertTrue(r.ok, "\(label): \(r.diagnostics)")
                    XCTAssertEqual(r.inactiveBindings, [], label)
                    let preset = try XCTUnwrap(CoreEngine.resolvedOpts(state: r.state, size: r.size)).speed
                    let got = CoreEngine.frameFromFxSpecWith(
                        json: json, elapsed: 1.3, state: key, inputs: Self.inputs, lowPower: low)
                    let want = CoreEngine.frameWithOverrides(
                        state: r.state, size: r.size, t: 1.3 * preset * r.speed, overrides: r.overrides)
                    XCTAssertNotNil(got, label)
                    XCTAssertEqual(got, want, label)
                    // The cost estimate is available natively too.
                    XCTAssertNotNil(
                        CoreEngine.fxSpecCost(json: json, state: key, inputs: Self.inputs, lowPower: low), label)
                }
            }
        }
        // FX Spec 1.2: low power caps fps and sheds glow + noise.
        let power = try String(
            contentsOf: dir.appendingPathComponent("status-beacon-power.fxspec.json"), encoding: .utf8)
        XCTAssertEqual(CoreEngine.resolveFxSpecWith(json: power, state: nil, inputs: [:]).maxFps, 30)
        let low = CoreEngine.resolveFxSpecWith(json: power, state: nil, inputs: [:], lowPower: true)
        XCTAssertEqual(low.maxFps, 15)
        XCTAssertEqual(low.disabledMaterials, ["glow", "noise"])
        XCTAssertEqual(
            CoreEngine.estimateCost(state: "working", size: 64, overrides: ["glowStrength": 1])?.class, "heavy")
        // Per-state particle defaults (the Studio's knob defaults).
        XCTAssertEqual(CoreEngine.particleDefaults(state: "tracking")?["particleStyle"], 3)
        XCTAssertEqual(CoreEngine.particleDefaults(state: "notifying")?["particleSync"], 1)
        XCTAssertEqual(CoreEngine.particleDefaults(state: "working")?["particleLife"], 4.5)
        XCTAssertNil(CoreEngine.particleDefaults(state: "nope"))
    }

    private static let inputs: [String: Double] = [
        "micMuted": 0, "micLevel": 0.6, "agentVolume": 0.5, "steps": 6200, "waterMl": 1800,
        "activeMinutes": 12, "heartRate": 128,
    ]

    func testDiagnosticsAndColor() {
        let r = CoreEngine.resolveFxSpec(
            json: #"{"fxSpec":"1.8","object":"orb","pattern":"working","params":{"orbitn":3}}"#)
        XCTAssertFalse(r.ok)
        XCTAssertTrue(r.diagnostics.contains { $0.path == "/params/orbitn" && $0.message.contains("`orbitN`") })
        XCTAssertNil(
            CoreEngine.frameFromFxSpec(json: #"{"fxSpec":"2.0","object":"orb","pattern":"working"}"#, elapsed: 0))
        let c = CoreEngine.fxColorToHsl(color: "#ff00ff")
        XCTAssertEqual(c?.hex, "#ff00ff")
        XCTAssertEqual(c?.h ?? -1, 300, accuracy: 1e-9)
    }

    /// The parameter catalog (docs/parameters.md) crosses UniFFI as JSON text;
    /// checkOverrides returns warnings only.
    func testParameterCatalogAndCheckOverrides() throws {
        let data = CoreEngine.parameterCatalogJson().data(using: .utf8)!
        let cat = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let objects = try XCTUnwrap(cat["objects"] as? [[String: Any]])
        XCTAssertEqual(
            objects.compactMap { $0["component"] as? String },
            ["SinuaOrb", "SinuaSignal", "SinuaRing", "SinuaCore", "SinuaBeacon"])
        let defs = try XCTUnwrap(cat["definitions"] as? [String: [String: Any]])
        XCTAssertEqual(defs["glowStrength@shared"]?["path"] as? String, "glow.strength")
        let w = CoreEngine.checkOverrides(state: "breathing", size: 64, overrides: ["lanse": 6])
        XCTAssertEqual(w.count, 1)
        XCTAssertEqual(w.first?.severity, "warning")
        XCTAssertTrue(w.first?.message.contains("did you mean `lanes`") ?? false)
        XCTAssertTrue(CoreEngine.checkOverrides(state: "tracking", size: 64, overrides: ["progress1": 2]).isEmpty)
    }
}
