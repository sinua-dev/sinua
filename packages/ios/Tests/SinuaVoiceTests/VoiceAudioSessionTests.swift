import AVFoundation
import XCTest

@testable import SinuaVoice

/// Design note 30 (V1/V2): the session policy and its ownership, with fakes only. No
/// test here touches the real audio session (the simulator's is the Mac's).
final class VoiceAudioSessionTests: XCTestCase {
    func testTheSpeakerPolicyIsVideoChatOnTheLoudspeakerAndTheReceiverIsVoiceChat() throws {
        let speaker = try XCTUnwrap(VoiceAudioSession.speaker.policy)
        XCTAssertEqual(speaker.category, .playAndRecord)
        XCTAssertEqual(speaker.mode, .videoChat)
        XCTAssertTrue(speaker.options.contains(.defaultToSpeaker))
        XCTAssertTrue(speaker.options.contains(.allowBluetooth))
        let receiver = try XCTUnwrap(VoiceAudioSession.receiver.policy)
        XCTAssertEqual(receiver.mode, .voiceChat)
        XCTAssertFalse(receiver.options.contains(.defaultToSpeaker))
        XCTAssertNil(VoiceAudioSession.unmanaged.policy)
    }

    func testTwoSourcesShareOneActivationAndTheLastOneDeactivates() throws {
        let claims = AudioSessionClaims()
        var log: [String] = []
        try claims.claim { log.append("activate") }  // source A connects
        try claims.claim { log.append("activate") }  // source B connects: already active
        claims.release { log.append("deactivate") }  // A disconnects: B still plays
        XCTAssertEqual(log, ["activate"])
        claims.release { log.append("deactivate") }  // B disconnects: the last one
        XCTAssertEqual(log, ["activate", "deactivate"])
        claims.release { log.append("deactivate") }  // an extra release does nothing
        XCTAssertEqual(log, ["activate", "deactivate"])
    }

    func testAFailedActivationClaimsNothing() {
        struct Busy: Error {}
        let claims = AudioSessionClaims()
        var log: [String] = []
        XCTAssertThrowsError(try claims.claim { throw Busy() })
        claims.release { log.append("deactivate") }
        XCTAssertEqual(log, [], "nothing to release after a failed claim")
        XCTAssertNoThrow(try claims.claim { log.append("activate") })
        XCTAssertEqual(log, ["activate"], "the next claim activates")
    }
}
