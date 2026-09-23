import XCTest

@testable import SinuaVoice

/// The real `AVPcmAudioDevice` on whatever audio the simulator has: it starts,
/// its player clock advances, a buffer scheduled on that clock is accepted,
/// the reset restarts the clock, and it stops cleanly. Not a sound-quality or
/// echo-cancellation check (that needs a device).
///
/// **Opt-in only.** On a simulator the engine is the Mac's real audio: it
/// opens the Mac microphone (a macOS permission prompt) and drives the
/// speakers. Four sessions share this machine and the user is at it, so the
/// default suite skips these; run them deliberately with
/// `TEST_RUNNER_SINUA_AUDIO_SMOKE=1 xcodebuild test ...`. The scheduled
/// buffer is silence, so even an opted-in run plays nothing audible.
final class AVPcmAudioDeviceSmokeTests: XCTestCase {
    func testStartsSchedulesResetsAndStops() throws {
        try run(AVPcmAudioDevice(configureSession: true, voiceProcessing: false))
    }

    /// The default configuration apps get (Apple's echo canceller on).
    func testWithVoiceProcessing() throws {
        try run(AVPcmAudioDevice())
    }

    private func run(_ dev: AVPcmAudioDevice) throws {
        guard ProcessInfo.processInfo.environment["SINUA_AUDIO_SMOKE"] == "1" else {
            throw XCTSkip("opens the real mic/speakers; set TEST_RUNNER_SINUA_AUDIO_SMOKE=1 to run")
        }
        do {
            try dev.start(inputRate: 16000, outputRate: 24000) { _ in }
        } catch {
            throw XCTSkip("simulator audio unavailable: \(error)")
        }
        defer { dev.stop() }
        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        let t0 = dev.playedFrames
        XCTAssertGreaterThan(t0, 0, "the player clock runs")
        dev.schedule([Float](repeating: 0, count: 2400), atFrame: t0 + 2400)  // silence: nothing audible

        RunLoop.main.run(until: Date().addingTimeInterval(0.3))
        let t1 = dev.playedFrames
        XCTAssertGreaterThan(t1, t0 + 24000 / 10, "~0.3 s of frames at 24 kHz")
        dev.resetPlayback(fade: false)
        RunLoop.main.run(until: Date().addingTimeInterval(0.1))
        XCTAssertLessThan(dev.playedFrames, t1, "the clock restarted")
    }
}
