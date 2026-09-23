import CoreEngine
import SwiftUI
import XCTest

@testable import Sinua

/// Seam check for per-vertex stroke colour (docs/engine.md): a polyline whose
/// `hues` are all equal, painted through the per-vertex path (segments in one
/// layer, composited once at `a`), must look like the same polyline painted as
/// one stroke -- the no-seam reference. A deliberately naive per-segment
/// composite (each segment straight at `a`) is the negative control: its
/// round-cap overlaps double-paint, and the metric must catch that.
@MainActor
final class PerVertexSeamTests: XCTestCase {
    /// A zigzag with sharp bends and a repeated point, at alpha 0.5.
    private func polyline(hues: [Double]) -> Polyline {
        let pts = [(8.0, 40.0), (16, 12), (24, 44), (30, 20), (30, 20), (40, 48), (48, 10), (56, 36)]
            .map { Point(x: $0.0, y: $0.1) }
        return Polyline(points: pts, white: 0.5, a: 0.5, w: 3, saturation: 1, hue: 200, hues: hues)
    }

    private func frame(_ p: Polyline) -> OrbFrame {
        OrbFrame(dots: [], lines: [], polylines: [p], colorMode: .ink, fills: [], effects: [])
    }

    /// 256x256 RGBA composited over white (the tolerance metric's convention).
    private func render(_ draw: @escaping (inout GraphicsContext, CGSize) -> Void) throws -> [UInt8] {
        let view = Canvas { ctx, size in draw(&ctx, size) }.frame(width: 256, height: 256).background(Color.white)
        let r = ImageRenderer(content: view)
        r.scale = 1
        let img = try XCTUnwrap(r.cgImage)
        var px = [UInt8](repeating: 0, count: 256 * 256 * 4)
        let cs = CGColorSpace(name: CGColorSpace.sRGB)!
        let ctx = try XCTUnwrap(
            CGContext(
                data: &px, width: 256, height: 256, bitsPerComponent: 8, bytesPerRow: 256 * 4,
                space: cs, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ctx.draw(img, in: CGRect(x: 0, y: 0, width: 256, height: 256))
        return px
    }

    /// Channel-diff mean / p99 / max, plus how many *pixels* differ by more than 24/255.
    private func stats(_ a: [UInt8], _ b: [UInt8]) -> (mean: Double, p99: Double, max: Double, over24: Int) {
        var d: [Double] = []
        d.reserveCapacity(256 * 256 * 3)
        var over = 0
        for i in stride(from: 0, to: a.count, by: 4) {
            var px = 0.0
            for c in 0..<3 {
                let v = abs(Double(a[i + c]) - Double(b[i + c]))
                d.append(v)
                px = max(px, v)
            }
            if px > 24 { over += 1 }
        }
        d.sort()
        return (d.reduce(0, +) / Double(d.count), d[Int(Double(d.count) * 0.99)], d.last ?? 0, over)
    }

    func testConstantHuesMatchOneStrokeAndNaiveSegmentsDoNot() throws {
        let one = frame(polyline(hues: []))
        let layered = frame(polyline(hues: Array(repeating: 200, count: 8)))
        XCTAssertTrue(FxPaint.hasHues(layered.polylines[0]))
        let ref = try render { ctx, size in FxPaint.draw(one, into: &ctx, size: size, engineSize: 64, dark: false) }
        let got = try render { ctx, size in FxPaint.draw(layered, into: &ctx, size: size, engineSize: 64, dark: false) }
        let naive = try render { ctx, size in
            ctx.scaleBy(x: size.width / 64, y: size.width / 64)
            let p = layered.polylines[0]
            let color = FxPaint.ink(white: p.white, saturation: p.saturation, hue: 200, alpha: p.a, dark: false)
            for i in 1..<p.points.count {
                var seg = Path()
                seg.move(to: CGPoint(x: p.points[i - 1].x, y: p.points[i - 1].y))
                seg.addLine(to: CGPoint(x: p.points[i].x, y: p.points[i].y))
                ctx.stroke(seg, with: .color(color), style: StrokeStyle(lineWidth: p.w, lineCap: .round))
            }
        }
        let layeredStats = stats(ref, got)
        let naiveStats = stats(ref, naive)
        print(
            "SEAM layered vs one-stroke: mean \(layeredStats.mean) p99 \(layeredStats.p99) max \(layeredStats.max) px>24 \(layeredStats.over24)"
        )
        print(
            "SEAM naive   vs one-stroke: mean \(naiveStats.mean) p99 \(naiveStats.p99) max \(naiveStats.max) px>24 \(naiveStats.over24)"
        )
        XCTAssertLessThanOrEqual(layeredStats.mean, 2)
        XCTAssertLessThanOrEqual(layeredStats.p99, 24)
        // The negative control: every joint's cap overlap double-paints at a = 0.5.
        XCTAssertGreaterThan(naiveStats.max, 40, "the negative control must show the double-painted joints")
        // What's left in the layered render is anti-aliasing on the outline only:
        // where two segments' AA edges overlap inside the layer their coverages
        // combine (1-(1-c1)(1-c2)), a touch darker than one stroker's coverage --
        // a few outline pixels at bends, not the notch. Measured 31 vs 1044 px.
        XCTAssertLessThan(
            layeredStats.over24 * 10, naiveStats.over24, "no seam: far fewer differing pixels than the naive composite")
        for (name, px) in [("seam-ref", ref), ("seam-layered", got), ("seam-naive", naive)] {
            attach(name, png(px))
        }
    }

    /// 16x zoom of `tracking-64-0.6-holo`'s tracks (alpha < 1), light and dark, for a visual seam look.
    func testZoomedTrackingHoloAttachment() throws {
        let f = try XCTUnwrap(frameWithOverrides(state: "tracking", size: 64, t: 0.6, overrides: ["holoStrength": 1]))
        XCTAssertTrue(f.polylines.contains { FxPaint.hasHues($0) }, "the engine emits hues here")
        for dark in [false, true] {
            let px = try render { ctx, size in
                ctx.translateBy(x: -2 * size.width, y: 0)  // a quarter of the frame at 16x: engine x 8..24
                FxPaint.draw(
                    f, into: &ctx, size: CGSize(width: size.width * 4, height: size.height * 4), engineSize: 64,
                    dark: dark)
            }
            attach("ios-seam-zoom-tracking-holo-\(dark ? "dark" : "light")", png(px))
        }
    }

    private func png(_ px: [UInt8]) -> Data {
        var copy = px
        let cs = CGColorSpace(name: CGColorSpace.sRGB)!
        let ctx = CGContext(
            data: &copy, width: 256, height: 256, bitsPerComponent: 8, bytesPerRow: 256 * 4,
            space: cs, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue)!
        return UIImage(cgImage: ctx.makeImage()!).pngData()!
    }

    private func attach(_ name: String, _ data: Data) {
        let a = XCTAttachment(data: data, uniformTypeIdentifier: "public.png")
        a.name = "\(name).png"
        a.lifetime = .keepAlways
        add(a)
    }
}
