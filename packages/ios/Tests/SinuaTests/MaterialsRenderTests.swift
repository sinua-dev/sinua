import CoreEngine
import SwiftUI
import XCTest

@testable import Sinua

/// Renders families' materials golden cases (spec/sinua-golden.json 1.2.0)
/// at 256 px, light and dark, as test attachments. The cross-platform
/// tolerance comparison against Web/Android runs outside (docs/fx-view.md).
@MainActor
final class MaterialsRenderTests: XCTestCase {
    static let cases: [(key: String, state: String, overrides: [String: Double])] = [
        ("generating-64-0.6-highlight-fill", "generating", ["highlightFill": 1]),
        ("scanning-64-0.6-trail-fill", "scanning", ["trailFill": 1]),
        ("glowing-64-0.6-glow-blur", "glowing", ["glowMode": 1, "glowStrength": 1]),
        ("tracking-64-0.6-glow-blur-additive", "tracking", ["glowBlend": 1, "glowMode": 1, "glowStrength": 0.8]),
        // Materials phase 2 (Liquid): only the fill style uses a new paint concept (holes, even-odd).
        ("speaking-64-0.6-liquid-fill", "speaking", ["liquidStrength": 1, "liquidStyle": 0]),
        ("metering-64-0.6-liquid-outline", "metering", ["liquidStrength": 1]),
        ("glowing-64-0.6-liquid-dots", "glowing", ["liquidStrength": 1, "liquidStyle": 2]),
        // Materials phase 3 (Particles): plain Dots -- no new paint concept.
        ("glowing-64-0.6-particles-drift", "glowing", ["particleStrength": 1, "particleStyle": 0]),
        ("speaking-64-0.6-particles-attract", "speaking", ["particleStrength": 1, "particleStyle": 1]),
        ("completing-64-0.6-particles-orbit", "completing", ["particleStrength": 1, "particleStyle": 2]),
        ("drifting-64-0.6-particles-liquid", "drifting", ["liquidStrength": 1, "particleStrength": 1]),
        // Materials phase 4 (holographic-lite): hue/saturation only -- no new paint concept.
        ("glowing-64-0.6-holo", "glowing", ["holoStrength": 1]),
        ("speaking-64-0.6-holo-fill", "speaking", ["holoStrength": 1, "liquidStrength": 1, "liquidStyle": 0]),
        ("completing-64-0.6-holo-glow", "completing", ["holoStrength": 1, "glowStrength": 0.8]),
        ("drifting-64-0.6-holo-gradient", "drifting", ["holoStrength": 0.5, "gradientStrength": 1]),
        // Per-vertex stroke colour (golden 1.6.0, Polyline.hues): the one-layer paint rule.
        ("tracking-64-0.6-holo", "tracking", ["holoStrength": 1]),
        (
            "tracking-64-0.6-gradient3", "tracking",
            ["gradientStrength": 1, "gradientHue": 200, "gradientHue2": 300, "gradientHue3": 40]
        ),
        ("locating-64-0.6-holo", "locating", ["holoStrength": 1]),
        ("completing-64-0.6-holo-interrupt", "completing", ["holoStrength": 1, "interruptAge": 0.15]),
        // A character is fills only (docs/character.md): celebrate and one everyday frame each.
        ("buzzy-64-0.6-celebrate", "buzzy", ["effectCode": 3, "effectAge": 0.5]),
        ("buzzy-64-0.6-muted", "buzzy", ["look": 0, "muted": 1]),
        ("hum-64-0.6-celebrate", "hum", ["effectCode": 3, "effectAge": 0.5]),
        ("hum-64-0.6-barge-in", "hum", ["look": 0, "interruptAge": 0.05]),
        ("wisp-64-0.6-celebrate", "wisp", ["effectCode": 3, "effectAge": 0.5]),
        ("wisp-64-0.6-barge-in", "wisp", ["look": 0, "interruptAge": 0.05]),
        ("chirp-64-0.6-celebrate", "chirp", ["effectCode": 3, "effectAge": 0.5]),
        ("chirp-64-0.6-barge-in", "chirp", ["look": 0, "interruptAge": 0.05]),
        ("buzzy-64-0.6-turned", "buzzy", ["turnYaw": -0.7, "turnPitch": 0.8, "gazeX": -8, "gazeY": -4, "look": 0]),
        ("chirp-64-0.6-turned", "chirp", ["turnYaw": -0.7, "turnPitch": 0.8, "gazeX": -8, "gazeY": -4, "look": 0]),
        ("cuppa-64-0.6-celebrate", "cuppa", ["effectCode": 3, "effectAge": 0.5]),
        ("bean-64-0.6-turned", "bean", ["turnYaw": -0.7, "turnPitch": 0.8, "gazeX": -8, "gazeY": -4, "look": 0]),
        ("beep-64-0.6-celebrate", "beep", ["effectCode": 3, "effectAge": 0.5]),
        // Synthetic, information only (packages/web/scripts/materials/frames.mjs SYNTHETIC):
        // per-vertex strokes under a blur / additive run at the composite.
        ("x-completing-64-0.6-holo-glowblur", "completing", ["holoStrength": 1, "glowStrength": 0.8, "glowMode": 1]),
        (
            "x-completing-64-0.6-holo-glowblur-additive", "completing",
            ["holoStrength": 1, "glowStrength": 0.8, "glowMode": 1, "glowBlend": 1]
        ),
    ]

    /// FX Spec rows (1.13, design note 22; frames.mjs SPEC_ROWS): the showcase examples
    /// resolved at their base design -- elliptical gradients, soft layers, rims, grain.
    static let specRows: [(key: String, file: String)] = [
        ("rich-bean-64-0.6-spec", "rich-bean.fxspec.json"),
        ("rich-buzzy-64-0.6-spec", "rich-buzzy.fxspec.json"),
    ]

    func testRenderMaterialsGoldenCases() throws {
        let golden = try goldenOverrides()
        var frames: [(String, OrbFrame)] = []
        for c in Self.cases {
            let overrides = golden[c.key] ?? c.overrides
            frames.append(
                (
                    c.key,
                    try XCTUnwrap(frameWithOverrides(state: c.state, size: 64, t: 0.6, overrides: overrides), c.key)
                ))
        }
        for row in Self.specRows {
            let json = try String(contentsOf: specURL("examples/\(row.file)"), encoding: .utf8)
            let r = resolveFxSpecWith(json: json, state: "idle", inputs: [:])
            XCTAssertTrue(r.ok, "\(row.file): \(r.diagnostics)")
            let frame = try XCTUnwrap(
                frameWithOverrides(state: r.state, size: 64, t: 0.6, overrides: r.overrides), row.key)
            XCTAssertTrue(frame.fills.contains { $0.blend == 2 }, "\(row.key) has grain")
            XCTAssertTrue(frame.fills.contains { $0.gradient?.kind == 2 }, "\(row.key) has an elliptical gradient")
            frames.append((row.key, frame))
        }
        for (key, frame) in frames {
            if !key.contains("liquid-outline") && !key.contains("liquid-dots") && !key.contains("particles")
                && !key.contains("holo") && !key.contains("gradient3")
            {
                XCTAssertTrue(!frame.fills.isEmpty || !frame.effects.isEmpty, "\(key) has materials")
            }
            if key.contains("liquid-fill") {
                XCTAssertFalse(frame.fills.allSatisfy { $0.holes.isEmpty }, "the liquid fill has a hole")
            }
            for dark in [false, true] {
                let view = Canvas { ctx, size in FxPaint.draw(frame, into: &ctx, size: size, engineSize: 64, dark: dark)
                }
                .frame(width: 256, height: 256)
                let r = ImageRenderer(content: view)
                r.scale = 1
                let png = try XCTUnwrap(r.uiImage?.pngData())
                let a = XCTAttachment(data: png, uniformTypeIdentifier: "public.png")
                a.name = "ios-\(key)-\(dark ? "dark" : "light").png"
                a.lifetime = .keepAlways
                add(a)
            }
        }
    }

    /// The grain tile's hash (design note 22) matches the vectors every platform checks
    /// (packages/web/test/materials.test.mjs, GrainVectorsTest.kt).
    func testGrainHashMatchesTheVectors() {
        let want: [(Int, Int, Double)] = [
            (0, 0, 0.573750742), (1, 0, 0.78714704), (0, 1, 0.399833626), (63, 63, 0.680704963), (17, 42, 0.127631493),
        ]
        for (x, y, v) in want { XCTAssertEqual(FxPaint.grainValue(x, y), v, accuracy: 1e-9, "\(x),\(y)") }
        XCTAssertNotNil(FxPaint.grainTile)
    }

    /// A file under the repository's `spec/`.
    private func specURL(_ path: String) -> URL {
        URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("spec/\(path)")
    }

    /// The overrides recorded in the golden file (so the frames match families' cases exactly).
    private func goldenOverrides() throws -> [String: [String: Double]] {
        let url = specURL("sinua-golden.json")
        guard let data = try? Data(contentsOf: url),
            let d = try JSONSerialization.jsonObject(with: data) as? [String: Any],
            let cases = d["cases"] as? [[String: Any]]
        else { return [:] }
        var out: [String: [String: Double]] = [:]
        for c in cases {
            if let k = c["key"] as? String, let o = c["overrides"] as? [String: NSNumber] {
                out[k] = o.mapValues(\.doubleValue)
            }
        }
        return out
    }
}
