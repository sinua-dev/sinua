import SwiftUI

// The paint contract, moved verbatim from the iOS Studio's OrbCanvasView.swift
// (`OrbPaint`, now a typealias of this). Shared by SinuaView and the Studio's
// live canvas, thumbnails, transitions and PNG export, so none can drift.

/// Paint contract (spec/orbs-spec.json, plus `Polyline`'s doc comment in
/// crates/core_engine/src/primitives.rs): polylines draw first, then lines,
/// then dots; plain source-over; ink mirrors on dark themes -- grey =
/// (dark ? 1-white : white). Ported 1:1 from the Web Studio's canvas/
/// drawFrame.ts so this view and `renderFrame(to:)` (used by PNG export)
/// can never drift from each other, the same way the Web Studio shares one
/// `drawFrame` between its live canvas and its PNG exporter.
public enum FxPaint {
    /// drawFrame.ts's `ink()`: grayscale when `saturation` is 0 (every
    /// ported `orbs` mode), else HSL with `white` as lightness. SwiftUI's
    /// `Color(hue:saturation:brightness:)` is HSB, not HSL, so the
    /// conversion is done by hand to match the Web/Android/SVG output
    /// exactly rather than approximately.
    public static func ink(white: Double, saturation: Double = 0, hue: Double = 0, alpha: Double, dark: Bool) -> Color {
        let w = min(1, max(0, white))
        let l = dark ? 1 - w : w
        if saturation <= 0 {
            return Color(red: l, green: l, blue: l, opacity: alpha)
        }
        let (r, g, b) = hslToRgb(h: hue, s: min(1, max(0, saturation)), l: l)
        return Color(red: r, green: g, blue: b, opacity: alpha)
    }

    /// Standard HSL -> RGB (the CSS `hsl()` definition), all components 0...1.
    public static func hslToRgb(h: Double, s: Double, l: Double) -> (Double, Double, Double) {
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

    public static func draw(
        _ frame: OrbFrame, into context: inout GraphicsContext, size: CGSize, engineSize: Double, dark: Bool
    ) {
        // Effect runs (blur, additive, light): their own path, so frames
        // without them keep exactly the drawing below (bitmap parity).
        if !frame.effects.isEmpty {
            drawWithMaterials(frame, into: &context, size: size, engineSize: engineSize, dark: dark)
            return
        }
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

    // MARK: - Effect runs (contract: docs/engine.md "Paint contract")

    /// SwiftUI `.blur(radius:)` -> Gaussian sigma: `radius = blurRadiusPerSigma * sigma`
    /// (in the context's user space). Measured against the Web renderer's
    /// sigma-exact blur -- see docs/fx-view.md, *Effects*.
    public static var blurRadiusPerSigma: Double = 1.0

    /// Applies an element's blur (Gaussian sigma, engine units) and additive blend to a copy of
    /// the context; `draw` paints into it. The copy keeps the caller's opacity (cross-dissolves).
    private static func withEffect(
        _ context: GraphicsContext, blur: Double, blend: UInt8, _ draw: (inout GraphicsContext) -> Void
    ) {
        var c = context
        if blend == 1 { c.blendMode = .plusLighter }
        if blur > 0 { c.addFilter(.blur(radius: blur * blurRadiusPerSigma)) }
        draw(&c)
    }

    /// `blend` 3 on dots (1.15 light volumes, docs/engine.md *Light*): each dot is a soft radial
    /// glow of radius `r` -- a bright core, its colour, then nothing -- added up on a dark
    /// background (`plusLighter`). A light background can't take added light, so there the glow,
    /// its core no brighter than the colour, is laid on normally. The Web's `drawLight`.
    static let light: UInt8 = 3

    private static func drawLight(_ d: Dot, in context: GraphicsContext, dark: Bool, mirror: Bool) {
        let l = mirror ? 1 - d.white : d.white
        guard d.r > 0, let sprite = lightSprite(hue: d.hue, saturation: d.saturation, lightness: l, dark: dark) else {
            return
        }
        var c = context
        c.blendMode = dark ? .plusLighter : .normal
        c.opacity = min(1, max(0, d.a))
        c.draw(
            Image(decorative: sprite, scale: 1),
            in: CGRect(x: d.x - d.r, y: d.y - d.r, width: d.r * 2, height: d.r * 2))
    }

    /// One 32-px glow per colour (hue, saturation and lightness in whole steps), drawn once.
    private static var lightSprites: [Int: CGImage] = [:]

    private static func lightSprite(hue: Double, saturation: Double, lightness: Double, dark: Bool) -> CGImage? {
        let h = Int(hue.rounded())
        let s = Int((saturation * 100).rounded())
        let l = Int((lightness * 100).rounded())
        let key = (h << 24) | (s << 16) | (l << 8) | (dark ? 1 : 0)
        if let hit = lightSprites[key] { return hit }
        let core = dark ? min(0.97, Double(l) / 100 + 0.30) : max(0.10, Double(l) / 100 - 0.22)
        let tone = dark ? Double(l) / 100 : max(0.10, Double(l) / 100 - 0.22)
        let rgba = { (w: Double, a: Double) -> CGColor in
            let (r, g, b) = hslToRGB(Double(h), Double(s) / 100, w)
            return CGColor(srgbRed: r, green: g, blue: b, alpha: a)
        }
        let size = 32
        guard
            let ctx = CGContext(
                data: nil, width: size, height: size, bitsPerComponent: 8, bytesPerRow: 0,
                space: CGColorSpace(name: CGColorSpace.sRGB)!, bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue),
            let gradient = CGGradient(
                colorsSpace: CGColorSpace(name: CGColorSpace.sRGB),
                colors: [rgba(core, 1), rgba(tone, 0.9), rgba(tone, 0.22), rgba(tone, 0)] as CFArray,
                locations: [0, 0.18, 0.45, 1])
        else { return nil }
        let mid = CGPoint(x: Double(size) / 2, y: Double(size) / 2)
        ctx.drawRadialGradient(
            gradient, startCenter: mid, startRadius: 0, endCenter: mid, endRadius: Double(size) / 2, options: [])
        guard let image = ctx.makeImage() else { return nil }
        if lightSprites.count > 512 { lightSprites.removeAll() }
        lightSprites[key] = image
        return image
    }

    /// HSL (hue in degrees, saturation and lightness 0..1) to sRGB 0..1.
    private static func hslToRGB(_ h: Double, _ s: Double, _ l: Double) -> (Double, Double, Double) {
        let c = (1 - abs(2 * l - 1)) * s
        let hp = (h.truncatingRemainder(dividingBy: 360) + 360).truncatingRemainder(dividingBy: 360) / 60
        let x = c * (1 - abs(hp.truncatingRemainder(dividingBy: 2) - 1))
        let (r, g, b): (Double, Double, Double) =
            hp < 1
            ? (c, x, 0) : hp < 2 ? (x, c, 0) : hp < 3 ? (0, c, x) : hp < 4 ? (0, x, c) : hp < 5 ? (x, 0, c) : (c, 0, x)
        let m = l - c / 2
        return (r + m, g + m, b + m)
    }

    private static func runs(_ effects: [EffectRun], target: UInt8, count: Int) -> [EffectRun?]? {
        var map: [EffectRun?]?
        for e in effects where e.target == target && e.count > 0 {
            if map == nil { map = Array(repeating: nil, count: count) }
            let end = min(count, Int(e.start) + Int(e.count))
            if Int(e.start) < end { for i in Int(e.start)..<end { map![i] = e } }
        }
        return map
    }

    private static func drawWithMaterials(
        _ frame: OrbFrame, into context: inout GraphicsContext, size: CGSize, engineSize: Double, dark: Bool
    ) {
        let mirror = dark && frame.colorMode != .fixed
        let scale = min(size.width, size.height) / engineSize
        context.scaleBy(x: scale, y: scale)

        for p in frame.polylines where p.points.count >= 2 {
            var path = Path()
            path.move(to: CGPoint(x: p.points[0].x, y: p.points[0].y))
            for pt in p.points.dropFirst() { path.addLine(to: CGPoint(x: pt.x, y: pt.y)) }
            context.stroke(
                path,
                with: .color(ink(white: p.white, saturation: p.saturation, hue: p.hue, alpha: p.a, dark: mirror)),
                style: StrokeStyle(lineWidth: p.w, lineCap: .round, lineJoin: .round))
        }
        let lineRuns = runs(frame.effects, target: 1, count: frame.lines.count)
        for (i, l) in frame.lines.enumerated() {
            var path = Path()
            path.move(to: CGPoint(x: l.x1, y: l.y1))
            path.addLine(to: CGPoint(x: l.x2, y: l.y2))
            let e = lineRuns?[i] ?? nil
            withEffect(context, blur: e?.blur ?? 0, blend: e?.blend ?? 0) { c in
                c.stroke(
                    path,
                    with: .color(ink(white: l.white, saturation: l.saturation, hue: l.hue, alpha: l.a, dark: mirror)),
                    lineWidth: l.w)
            }
        }
        let dotRuns = runs(frame.effects, target: 0, count: frame.dots.count)
        for (i, d) in frame.dots.enumerated() {
            let rect = CGRect(x: d.x - d.r, y: d.y - d.r, width: d.r * 2, height: d.r * 2)
            let e = dotRuns?[i] ?? nil
            if e?.blend == light {
                drawLight(d, in: context, dark: dark, mirror: dark && frame.colorMode != .fixed)
                continue
            }
            withEffect(context, blur: e?.blur ?? 0, blend: e?.blend ?? 0) { c in
                c.fill(
                    Path(ellipseIn: rect),
                    with: .color(
                        ink(
                            white: d.white, saturation: d.saturation, hue: d.hue, alpha: d.a,
                            dark: dark && frame.colorMode != .fixed)))
            }
        }
    }
}
