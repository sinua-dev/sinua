import SwiftUI
import XCTest

@testable import Sinua

/// Cross-language parity for the painter's colour conversion, against
/// `spec/paint-ink-vectors.json` (`packages/web/scripts/gen-ink-vectors.mjs`).
///
/// This is the check that was missing. `OldOrbPaint.swift` next door compares
/// this painter against a frozen copy of its *own* past — useful, but it says
/// nothing about whether Swift, Kotlin and the Web agree. They each convert
/// differently: the Web emits a CSS `hsla()` string and lets the **browser**
/// convert (rounding saturation and lightness to whole percent first), Kotlin
/// calls Compose's `Color.hsl`, and this file does the arithmetic **by hand**
/// because SwiftUI's `Color(hue:saturation:brightness:)` is HSB, not HSL.
///
/// The vectors are real Chrome's answers, cross-checked against the CSS Color 4
/// algorithm (max channel disagreement: 1/255).
final class InkVectorsTests: XCTestCase {
    private struct Case: Decodable {
        let white: Double
        let saturation: Double
        let hue: Double
        let alpha: Double
        let dark: Bool
        let rgba: [Double]
    }

    private struct Vectors: Decodable {
        let cases: [Case]
    }

    private func loadVectors() throws -> [Case] {
        // `#filePath` like VoiceGoldenTests: simulator runs can see the repo.
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent()
            .appendingPathComponent("spec/paint-ink-vectors.json")
        let data = try Data(contentsOf: url)
        return try JSONDecoder().decode(Vectors.self, from: data).cases
    }

    /// SwiftUI `Color` -> 0...255 sRGB, the way a rasteriser would see it.
    private func channels(_ color: Color) -> (Double, Double, Double, Double) {
        var r: CGFloat = 0
        var g: CGFloat = 0
        var b: CGFloat = 0
        var a: CGFloat = 0
        UIColor(color).getRed(&r, green: &g, blue: &b, alpha: &a)
        return (Double(r) * 255, Double(g) * 255, Double(b) * 255, Double(a))
    }

    func testInkMatchesTheWebVectors() throws {
        let cases = try loadVectors()
        XCTAssertGreaterThan(cases.count, 500, "the vector file looks truncated")

        var worst = 0.0
        var worstCase: Case?
        var worstAlpha = 0.0
        var overOne = 0
        for c in cases {
            let (r, g, b, a) = channels(
                FxPaint.ink(white: c.white, saturation: c.saturation, hue: c.hue, alpha: c.alpha, dark: c.dark))
            let d = max(abs(r - c.rgba[0]), abs(g - c.rgba[1]), abs(b - c.rgba[2]))
            if d > worst {
                worst = d
                worstCase = c
            }
            if d > 1 { overOne += 1 }
            worstAlpha = max(worstAlpha, abs(a - c.rgba[3]))
        }

        // The bound is measured, not guessed: the Web rounds saturation and
        // lightness to whole percent before the browser converts, and this
        // painter does not, so a small systematic difference is expected and
        // is the thing worth pinning. If this number moves, the painters have
        // drifted -- or somebody changed the rounding on one side only.
        // Measured 2026-09-20: worst 2.04/255 over 1176 cases. The bound is 3
        // rather than 2.04 so an ordinary rounding shift does not fail the
        // build, but it is nowhere near loose enough to hide a real divergence
        // -- a wrong hue sector or a swapped channel is tens of units, not two.
        XCTAssertLessThanOrEqual(
            worst, 3.0,
            "worst channel delta \(worst) at \(String(describing: worstCase))")
        XCTAssertLessThanOrEqual(worstAlpha, 0.01, "alpha should pass through unchanged")
        print(
            "ink parity: \(cases.count) cases, worst \(worst)/255, "
                + "\(overOne) case(s) over 1/255, worst alpha \(worstAlpha)")
    }
}
