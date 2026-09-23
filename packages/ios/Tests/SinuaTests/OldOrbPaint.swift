import CoreEngine
import SwiftUI

// Snapshot of the iOS Studio's OrbPaint from before the move (2026-09-18), kept as the parity baseline.
enum OldOrbPaint {
    /// drawFrame.ts's `ink()`: grayscale when `saturation` is 0 (every
    /// ported `orbs` mode), else HSL with `white` as lightness. SwiftUI's
    /// `Color(hue:saturation:brightness:)` is HSB, not HSL, so the
    /// conversion is done by hand to match the Web/Android/SVG output
    /// exactly rather than approximately.
    static func ink(white: Double, saturation: Double = 0, hue: Double = 0, alpha: Double, dark: Bool) -> Color {
        let w = min(1, max(0, white))
        let l = dark ? 1 - w : w
        if saturation <= 0 {
            return Color(red: l, green: l, blue: l, opacity: alpha)
        }
        let (r, g, b) = hslToRgb(h: hue, s: min(1, max(0, saturation)), l: l)
        return Color(red: r, green: g, blue: b, opacity: alpha)
    }

    /// Standard HSL -> RGB (the CSS `hsl()` definition), all components 0...1.
    static func hslToRgb(h: Double, s: Double, l: Double) -> (Double, Double, Double) {
        let c = (1 - abs(2 * l - 1)) * s
        let hp = ((h.truncatingRemainder(dividingBy: 360) + 360).truncatingRemainder(dividingBy: 360)) / 60
        let x = c * (1 - abs(hp.truncatingRemainder(dividingBy: 2) - 1))
        let rgb1: (Double, Double, Double)
        switch hp {
        case ..<1: rgb1 = (c, x, 0)
        case ..<2: rgb1 = (x, c, 0)
        case ..<3: rgb1 = (0, c, x)
        case ..<4: rgb1 = (0, x, c)
        case ..<5: rgb1 = (x, 0, c)
        default: rgb1 = (c, 0, x)
        }
        let m = l - c / 2
        return (rgb1.0 + m, rgb1.1 + m, rgb1.2 + m)
    }

    static func draw(
        _ frame: OrbFrame, into context: inout GraphicsContext, size: CGSize, engineSize: Double, dark: Bool
    ) {
        // Colour mode (2026-09-18): "ink" frames mirror lightness on dark,
        // "fixed" frames keep `white` as-is in both themes.
        let mirror = dark && frame.colorMode != .fixed
        let scale = min(size.width, size.height) / engineSize
        context.scaleBy(x: scale, y: scale)

        // One stroked path per polyline, round caps + round joins -- never
        // one stroke per segment, which is the seam problem this primitive
        // exists to fix (see its Rust doc comment).
        for p in frame.polylines where p.points.count >= 2 {
            var path = Path()
            path.move(to: CGPoint(x: p.points[0].x, y: p.points[0].y))
            for pt in p.points.dropFirst() {
                path.addLine(to: CGPoint(x: pt.x, y: pt.y))
            }
            let style = StrokeStyle(lineWidth: p.w, lineCap: .round, lineJoin: .round)
            context.stroke(
                path, with: .color(ink(white: p.white, saturation: p.saturation, hue: p.hue, alpha: p.a, dark: mirror)),
                style: style)
        }
        // `Line`s keep the upstream contract's default butt caps.
        for l in frame.lines {
            var path = Path()
            path.move(to: CGPoint(x: l.x1, y: l.y1))
            path.addLine(to: CGPoint(x: l.x2, y: l.y2))
            context.stroke(
                path, with: .color(ink(white: l.white, saturation: l.saturation, hue: l.hue, alpha: l.a, dark: mirror)),
                lineWidth: l.w)
        }
        for d in frame.dots {
            let rect = CGRect(x: d.x - d.r, y: d.y - d.r, width: d.r * 2, height: d.r * 2)
            context.fill(
                Path(ellipseIn: rect),
                with: .color(ink(white: d.white, saturation: d.saturation, hue: d.hue, alpha: d.a, dark: mirror)))
        }
    }
}
