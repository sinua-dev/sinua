import CoreEngine
import XCTest

/// One-shot effects through the Swift bindings: spec/effect-vectors.json (the file
/// Web and Android read too), summarised the same way. docs/fx-view.md, *One-shot effects*.
final class EffectVectorsTests: XCTestCase {
    private func vectors() throws -> [String: Any] {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("spec/effect-vectors.json")
        guard let data = try? Data(contentsOf: url) else {
            throw XCTSkip("spec/effect-vectors.json not readable at \(url.path) (device run?)")
        }
        return try JSONSerialization.jsonObject(with: data) as! [String: Any]
    }

    private func summary(_ f: OrbFrame) -> [String: Double] {
        var alpha = 0.0
        var hue = 0.0
        var nh = 0.0
        var sx = 0.0
        var n = 0.0
        for d in f.dots {
            alpha += d.a
            if d.saturation > 0 {
                hue += d.hue
                nh += 1
            }
            sx += d.x
            n += 1
        }
        for l in f.lines {
            alpha += l.a
            if l.saturation > 0 {
                hue += l.hue
                nh += 1
            }
            sx += l.x1 + l.x2
            n += 2
        }
        for p in f.polylines {
            alpha += p.a
            if p.saturation > 0 {
                hue += p.hue
                nh += 1
            }
            for q in p.points {
                sx += q.x
                n += 1
            }
        }
        return [
            "dots": Double(f.dots.count), "lines": Double(f.lines.count), "polylines": Double(f.polylines.count),
            "alpha": alpha, "hue": nh == 0 ? 0 : hue / nh, "cx": n == 0 ? 0 : sx / n,
        ]
    }

    func testEachEffectRendersAsTheEngineVectorsSay() throws {
        let v = try vectors()
        let size = UInt32(v["size"] as! Int)
        let t = v["t"] as! Double
        let cases = v["cases"] as! [[String: Any]]
        XCTAssertGreaterThanOrEqual(cases.count, 72)
        for c in cases {
            let info = try XCTUnwrap(effectInfo(name: c["effect"] as! String))
            let reduced = c["reduced"] as! Bool
            let f = try XCTUnwrap(
                frameWithOverrides(
                    state: c["pattern"] as! String, size: size, t: t,
                    overrides: [
                        "effectCode": Double(info.code), "effectAge": c["age"] as! Double,
                        "effectReduced": reduced ? 1 : 0,
                    ]))
            let got = summary(f)
            let want = c["summary"] as! [String: Any]
            let label = "\(c["pattern"]!) \(c["effect"]!) \(c["age"]!) \(reduced)"
            for k in ["dots", "lines", "polylines", "alpha", "hue", "cx"] {
                XCTAssertEqual(got[k]!, (want[k] as! NSNumber).doubleValue, accuracy: 1e-5, "\(label) \(k)")
            }
        }
        XCTAssertNil(effectInfo(name: "confetti"))
    }
}
