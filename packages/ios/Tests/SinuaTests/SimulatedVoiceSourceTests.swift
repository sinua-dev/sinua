import CoreEngine
import SinuaVoiceTypes
import XCTest

@testable import Sinua

/// The simulated conversation on iOS: the engine through UniFFI agrees with
/// spec/conversation-vectors.json (the file the Web and Android tests read too), and the
/// source plays the script as a VoiceSource. No audio session, no mic.
@MainActor
final class SimulatedVoiceSourceTests: XCTestCase {
    func testTheEngineMatchesTheSharedVectors() throws {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("spec/conversation-vectors.json")
        guard let data = try? Data(contentsOf: url) else { throw XCTSkip("vectors not readable") }
        let doc = try XCTUnwrap(JSONSerialization.jsonObject(with: data) as? [String: Any])
        let bands = UInt32(doc["bands"] as! Int)
        let cases = try XCTUnwrap(doc["cases"] as? [[String: Any]])
        XCTAssertGreaterThan(cases.count, 100)
        XCTAssertEqual(SimulatedVoiceSource.sampleNames, ["calendar", "quick-answer", "long-answer", "barge-in"])
        for c in cases {
            let script = try XCTUnwrap(conversationSample(name: c["sample"] as! String))
            let f = conversationAt(json: script, t: c["t"] as! Double, bandCount: bands)
            XCTAssertEqual(f.state, c["state"] as? String)
            XCTAssertEqual(Int(f.turn), c["turn"] as? Int)
            XCTAssertEqual(f.level, c["level"] as! Double, accuracy: 1e-9)
            for (b, w) in zip(f.bands, c["bands"] as! [Double]) { XCTAssertEqual(b, w, accuracy: 1e-9) }
        }
    }

    func testPlaysTheScriptWithOneBargeInFlash() async throws {
        let src = try SimulatedVoiceSource(sample: "barge-in", bands: 8)
        var states: [AgentState] = []
        var interrupts = 0
        var last: VoiceMetrics?
        src.onStateChange { states.append($0) }
        src.onInterrupt { interrupts += 1 }
        src.onMetrics { last = $0 }
        try await src.connect()
        src.pause()  // stop the timer: drive time by hand (advance needs playing, so play around each step)
        for _ in 0..<Int(src.duration * 30) {
            src.play()
            src.advance(1.0 / 30)
            src.pause()
        }
        XCTAssertEqual(Array(states.prefix(5)), [.idle, .listening, .thinking, .speaking, .listening])
        XCTAssertEqual(interrupts, 1)
        XCTAssertEqual(last?.bands.count, 8)
        src.disconnect()
        XCTAssertEqual(states.last, .idle)
        XCTAssertEqual(last?.level, 0)
    }

    func testMutedSilencesTheUsersTurnsNotTheAgentsAndReportsTheConnection() async throws {
        let src = try SimulatedVoiceSource(sample: "calendar", loop: false)
        var last: VoiceMetrics?
        var conn: [Bool] = []
        src.onMetrics { last = $0 }
        src.onConnectionChange { conn.append($0) }
        try await src.connect()
        src.pause()
        let user = try XCTUnwrap(src.turns.first { $0.voice == "user" })
        let agent = try XCTUnwrap(src.turns.first { $0.voice == "agent" })
        src.seek(user.start + user.seconds / 2)
        XCTAssertGreaterThan(last?.level ?? 0, 0)
        src.setMuted(true)
        XCTAssertEqual(last?.level, 0)
        XCTAssertTrue(last?.bands.allSatisfy { $0 == 0 } ?? false)
        src.seek(agent.start + agent.seconds / 2)
        XCTAssertGreaterThan(last?.level ?? 0, 0, "the agent still talks")
        src.disconnect()
        XCTAssertEqual(conn, [true, false])
    }

    func testABadScriptThrowsWithItsPath() {
        XCTAssertThrowsError(try SimulatedVoiceSource(script: #"{"turns":[{"state":"idle","seconds":0}]}"#)) {
            XCTAssertTrue($0.localizedDescription.contains("/turns/0/seconds"), $0.localizedDescription)
        }
        XCTAssertThrowsError(try SimulatedVoiceSource(sample: "nope"))
    }

    func testTranscriptsFollowTheLinesWithTheBargeInCutTruncatedAndSubscribedBeforeConnect() async throws {
        let src = try SimulatedVoiceSource(sample: "barge-in", bands: 8)
        XCTAssertTrue(src.supportsTranscript)
        XCTAssertEqual(src.transcriptTiming, .synced)
        let shared = SharedVoiceSource.of(src)
        var got: [TranscriptUpdate] = []
        shared.listenTranscript { got.append($0) }
        try await shared.connect()
        src.pause()
        for _ in 0..<Int(src.duration * 30) {
            src.play()
            src.advance(1.0 / 30)
            src.pause()
        }
        var finals = got.filter(\.final)
        // The cut turn ends with what was said by its last frame: a prefix of its line (which frame that
        // is depends on the tick grid).
        XCTAssertEqual(finals.count, 4)
        let cut = finals[1]
        XCTAssertTrue(cut.truncated && cut.text.count >= 40, cut.text)
        XCTAssertTrue("The city museum opened in 1902 and holds over forty".hasPrefix(cut.text), cut.text)
        finals[1].text = "<cut>"
        XCTAssertEqual(
            finals,
            [
                TranscriptUpdate(role: .user, text: "Tell me about the museum.", final: true, turnId: "u1"),
                TranscriptUpdate(role: .assistant, text: "<cut>", final: true, turnId: "a1", truncated: true),
                TranscriptUpdate(role: .user, text: "Sorry, is it open today?", final: true, turnId: "u2"),
                TranscriptUpdate(role: .assistant, text: "Yes, until 6 pm.", final: true, turnId: "a2"),
            ])
        var seen: [String: TranscriptUpdate] = [:]
        for u in got {
            XCTAssertFalse(seen[u.turnId]?.final ?? false, "\(u.turnId) after final")
            if !u.truncated { XCTAssertTrue(u.text.hasPrefix(seen[u.turnId]?.text ?? "")) }
            seen[u.turnId] = u
        }
        src.disconnect()
    }
}
