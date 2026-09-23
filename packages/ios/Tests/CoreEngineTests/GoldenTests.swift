import XCTest

@testable import CoreEngine

/// Smoke-level parity check against `spec/orbs-golden.json`, run through the
/// real UniFFI FFI boundary (Swift -> C -> Rust -> C -> Swift). The
/// geometry itself is already exhaustively proven in
/// `crates/core_engine/tests/golden.rs` (all 9 modes, both sizes, 4
/// timestamps each); this only needs to prove the binding round-trips
/// values correctly, not re-derive correctness.
final class GoldenTests: XCTestCase {
    func testWorkingFrameMatchesGolden() throws {
        let frame = try XCTUnwrap(CoreEngine.frame(state: "working", size: 64, t: 0.6))
        let first = try XCTUnwrap(frame.dots.first)
        XCTAssertEqual(frame.dots.count, 516)
        XCTAssertEqual(first.x, 32.34438, accuracy: 1e-4)
        XCTAssertEqual(first.y, 30.683937, accuracy: 1e-4)
    }

    func testGlobeFrameMatchesGolden() throws {
        let frame = try XCTUnwrap(CoreEngine.frame(state: "searching", size: 64, t: 0.6))
        let first = try XCTUnwrap(frame.dots.first)
        XCTAssertEqual(frame.dots.count, 204)
        XCTAssertEqual(first.x, 33.490622, accuracy: 1e-4)
        XCTAssertEqual(first.y, 32.435651, accuracy: 1e-4)
    }

    func testWebFrameHasLinesAndMatchesGolden() throws {
        let frame = try XCTUnwrap(CoreEngine.frame(state: "connecting", size: 64, t: 0.6))
        let first = try XCTUnwrap(frame.dots.first)
        XCTAssertEqual(frame.dots.count, 48)
        XCTAssertEqual(frame.lines.count, 81)
        XCTAssertEqual(first.x, 23.99695, accuracy: 1e-4)
        XCTAssertEqual(first.y, 34.67833, accuracy: 1e-4)
    }

    func testMorphFrameMatchesGolden() throws {
        let frame = try XCTUnwrap(CoreEngine.frame(state: "shaping", size: 64, t: 0.6))
        let first = try XCTUnwrap(frame.dots.first)
        XCTAssertEqual(frame.dots.count, 24)
        XCTAssertEqual(first.x, 32, accuracy: 1e-4)
        XCTAssertEqual(first.y, 9.301059, accuracy: 1e-4)
    }

    func testAllNineStatesResolve() {
        let states = [
            "working", "searching", "solving", "listening", "connecting",
            "weaving", "composing", "breathing", "shaping",
        ]
        for state in states {
            XCTAssertNotNil(CoreEngine.frame(state: state, size: 64, t: 1.0), "\(state) should resolve")
        }
    }

    func testUnknownStateReturnsNil() {
        XCTAssertNil(CoreEngine.frame(state: "not-a-real-state", size: 64, t: 0))
    }

    /// Sanity check for `frameWithOverrides` (the Studio's live parameter
    /// sliders, now reachable from Swift too -- see
    /// crates/core_engine/tests/golden.rs's `frame_with_overrides_sanity`
    /// for the Rust-side equivalent this mirrors).
    func testFrameWithOverrides() throws {
        let stock = try XCTUnwrap(CoreEngine.frame(state: "searching", size: 64, t: 0.6))
        let empty = try XCTUnwrap(
            CoreEngine.frameWithOverrides(state: "searching", size: 64, t: 0.6, overrides: [:]))
        XCTAssertEqual(empty, stock, "empty overrides must reproduce the stock frame exactly")

        let overridden = try XCTUnwrap(
            CoreEngine.frameWithOverrides(state: "searching", size: 64, t: 0.6, overrides: ["scanMul": 8.0]))
        XCTAssertEqual(overridden.dots.count, stock.dots.count)
        XCTAssertNotEqual(overridden, stock)
    }
}
