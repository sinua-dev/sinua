import CoreEngine
import SwiftUI
import XCTest

@testable import Sinua

/// Roadmap 9/10: box-layout patterns fill the whole box, and a small view
/// defaults to 30 fps.
@MainActor
final class BoxLayoutTests: XCTestCase {
    private func config(_ input: FxInput, maxFps: Double? = nil) -> FxConfig {
        FxConfig(
            input: input, voice: nil, voiceOverrides: nil, specState: nil, inputs: [:], voiceLevelInput: nil,
            crossFade: 0, theme: .light, paused: false, reducedMotion: .never, label: nil, maxFps: maxFps,
            lowPower: .off, onFrame: nil)
    }

    func testTheEngineSaysWhichPatternsFillTheBox() {
        XCTAssertEqual(patternLayout(pattern: "framing"), "box")
        XCTAssertEqual(patternLayout(pattern: "playing"), "box")
        XCTAssertEqual(patternLayout(pattern: "breathing"), "square")
    }

    func testASmallViewDefaultsTo30FpsUnlessTheAppOrTheSpecSays() {
        let plain = FxInput.state("listening", 64, [:], 1)
        XCTAssertEqual(FxModel().performance(config(plain), lowPower: false, small: true).maxFps, 30)
        XCTAssertNil(
            FxModel().performance(config(plain), lowPower: false, small: false).maxFps, "a full-size view: display rate"
        )
        XCTAssertEqual(FxModel().performance(config(plain, maxFps: 60), lowPower: false, small: true).maxFps, 60)
        let spec = #"{"fxSpec":"1.8","object":"orb","pattern":"listening","performance":{"maxFps":60}}"#
        XCTAssertEqual(FxModel().performance(config(.spec(spec)), lowPower: false, small: true).maxFps, 60)
        let bare = #"{"fxSpec":"1.8","object":"orb","pattern":"listening"}"#
        XCTAssertEqual(FxModel().performance(config(.spec(bare)), lowPower: false, small: true).maxFps, 30)
    }

    /// Opaque pixels in the rightmost `band` columns of a rendered view.
    private func inkAtRightEdge(_ view: some View, width: CGFloat, height: CGFloat, band: Int = 6) throws -> Int {
        let r = ImageRenderer(content: view.frame(width: width, height: height))
        r.scale = 1
        let cg = try XCTUnwrap(r.cgImage)
        let data = try XCTUnwrap(cg.dataProvider?.data) as Data
        let bpp = cg.bitsPerPixel / 8
        var n = 0
        for y in 0..<cg.height {
            for x in (cg.width - band)..<cg.width {
                let alpha = data[y * cg.bytesPerRow + x * bpp + (cg.alphaInfo == .premultipliedFirst ? 0 : 3)]
                if alpha > 8 { n += 1 }
            }
        }
        return n
    }

    func testAnEdgeFillsAWideBoxAndASquarePatternStaysCentred() throws {
        let edge = SinuaView(
            pattern: "framing", overrides: ["idleOpacity": 0.8, "saturation": 0], reducedMotion: .never)
        XCTAssertGreaterThan(
            try inkAtRightEdge(edge, width: 400, height: 100), 50, "the rim reaches the box's right edge")
        let ring = SinuaView(pattern: "completing", overrides: ["progress": 1], reducedMotion: .never)
        XCTAssertEqual(try inkAtRightEdge(ring, width: 400, height: 100), 0, "a square pattern leaves the sides empty")
    }
}
