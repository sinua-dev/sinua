import CoreEngine
import SwiftUI
import XCTest

@testable import Sinua

/// SinuaAvatar and SinuaVoiceMessage (roadmap 7/8 helpers).
@MainActor
final class HelperViewsTests: XCTestCase {
    func testTheAvatarImageSpansTwiceTheRingsInnerRadius() {
        XCTAssertEqual(SinuaAvatar.defaultInnerRadius, 0.34)
        XCTAssertEqual(SinuaAvatar.imageDiameter(side: 100, innerRadius: 0.34), 68, accuracy: 1e-9)
        XCTAssertEqual(SinuaAvatar.imageDiameter(side: 56, innerRadius: 0.4), 44.8, accuracy: 1e-9)
    }

    func testSeekMapsATouchOntoTheEnginesRow() {
        let box = CGSize(width: 200, height: 40)
        XCTAssertEqual(SinuaVoiceMessage.seek(size: box, x: 0), 0)
        XCTAssertEqual(SinuaVoiceMessage.seek(size: box, x: 200), 1)
        XCTAssertEqual(SinuaVoiceMessage.seek(size: box, x: 100), 0.5, accuracy: 1e-9)
        XCTAssertEqual(SinuaVoiceMessage.seek(size: box, x: 100), playbackSeekProgress(aspect: 5, x: 2.5))
        XCTAssertEqual(SinuaVoiceMessage.seek(size: .zero, x: 10), 0)
    }

    func testTheVoiceMessagePassesItsEnvelopeAndProgressAsEngineKeys() {
        let m = SinuaVoiceMessage(envelope: Array(repeating: 0.5, count: 80), progress: 1.4)
        let keys = m.engineKeys
        XCTAssertEqual(keys["progress"], 1, "clamped")
        XCTAssertEqual(keys["envelope63"], 0.5)
        XCTAssertNil(keys["envelope64"], "at most 64 values")
    }
}
