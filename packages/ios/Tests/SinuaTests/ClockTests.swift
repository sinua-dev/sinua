import CoreEngine
import SwiftUI
import XCTest

@testable import Sinua

/// `PhaseClock`: the view clock is continuous across speed changes, so a lifecycle
/// state with its own speed (or an app changing `speed`) doesn't jump the pose.
/// The same cases as the Web mount tests and the Android ClockTest.
final class ClockTests: XCTestCase {
    private let frame = 1.0 / 60

    private func advance(_ clock: inout PhaseClock, _ frames: Int) {
        for _ in 0..<frames { clock.advance(frame) }
    }

    func testAConstantSpeedIsExactlyElapsedTimesSpeed() {
        var clock = PhaseClock()
        advance(&clock, 30)
        XCTAssertEqual(clock.phase(preset: 1.3, speed: 1.5), clock.elapsed * 1.3 * 1.5, accuracy: 0, "bit for bit")
    }

    func testASpeedChangeContinuesThePhaseInsteadOfJumping() {
        var clock = PhaseClock()
        advance(&clock, 120)  // two seconds in
        let before = clock.phase(preset: 1, speed: 1)
        clock.advance(frame)
        let after = clock.phase(preset: 1, speed: 3)
        XCTAssertEqual(after, before + frame * 3, accuracy: 1e-12, "one step at the new speed, from the phase it had")
        XCTAssertGreaterThan(abs(after - clock.elapsed * 3), 1, "not the rescaled elapsed")
    }

    func testSpeedZeroHoldsThePhaseAndResumesFromIt() {
        var clock = PhaseClock()
        advance(&clock, 60)
        let held = clock.phase(preset: 1, speed: 1)
        for _ in 0..<60 {
            clock.advance(frame)
            XCTAssertEqual(clock.phase(preset: 1, speed: 0), held, accuracy: 0, "frozen while speed is 0")
        }
        clock.advance(frame)
        XCTAssertEqual(clock.phase(preset: 1, speed: 1), held + frame, accuracy: 1e-12, "resumes from the held phase")
    }

    func testTheSpecPathDivisionReturnsAnElapsedThatMultipliesBackToThePhase() {
        var clock = PhaseClock()
        advance(&clock, 45)
        let phase = clock.phase(preset: 2, speed: 0.5)
        let at = phase / (2 * 0.5)  // what the view hands FxSpecPlayer
        XCTAssertEqual(at * 2 * 0.5, phase, accuracy: 1e-12)
    }
}

/// Voice-state profiles applied by the view for plain input (`pattern` + `state`),
/// the same cases as the Web mount tests and the Android VoiceStateTest.
@MainActor
final class VoiceStateProfileTests: XCTestCase {
    private func pixels<V: View>(_ view: V) throws -> Data {
        let r = ImageRenderer(
            content: ZStack {
                Color.white
                view
            }.frame(width: 96, height: 96))
        r.scale = 1
        let cg = try XCTUnwrap(r.cgImage)
        let ctx = try XCTUnwrap(
            CGContext(
                data: nil, width: cg.width, height: cg.height, bitsPerComponent: 8, bytesPerRow: cg.width * 4,
                space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        ctx.draw(cg, in: CGRect(x: 0, y: 0, width: cg.width, height: cg.height))
        return Data(bytes: try XCTUnwrap(ctx.data), count: ctx.bytesPerRow * cg.height)
    }

    func testTheProfileExistsForTheVoiceStatesOnly() {
        XCTAssertNotNil(voiceStateProfile(pattern: "working", state: "listening"))
        XCTAssertNil(voiceStateProfile(pattern: "working", state: "goalReached"), "an app's own state name")
        let listening = voiceStateProfile(pattern: "working", state: "listening")
        XCTAssertEqual(listening?.audioInput, "micLevel")
        XCTAssertLessThan(listening?.overrides["audioStrength"] ?? 0, 0, "listening draws inward")
    }

    func testAStateChangesWhatTheViewDrawsAndAnAppStateDoesNot() throws {
        let plain = try pixels(SinuaView(pattern: "working", paused: true))
        let listening = try pixels(SinuaView(pattern: "working", state: "listening", paused: true))
        let appState = try pixels(SinuaView(pattern: "working", state: "goalReached", paused: true))
        XCTAssertNotEqual(plain, listening, "the voice state moves the pattern")
        XCTAssertEqual(plain, appState, "an app's own state name is left alone")
    }

    func testTheAppsOwnOverridesWinOverTheProfile() throws {
        let profile = try XCTUnwrap(voiceStateProfile(pattern: "working", state: "listening"))
        let key = try XCTUnwrap(
            profile.overrides.keys.sorted().first { $0 == "glowStrength" } ?? profile.overrides.keys.sorted().first)
        let mine = (profile.overrides[key] ?? 0) + 0.3
        let withProfile = try pixels(SinuaView(pattern: "working", state: "listening", paused: true))
        let withMine = try pixels(
            SinuaView(pattern: "working", overrides: [key: mine], state: "listening", paused: true))
        XCTAssertNotEqual(withProfile, withMine, "my \(key) replaced the profile's")
    }
}
