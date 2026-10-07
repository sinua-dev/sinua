import XCTest

@testable import SinuaVoice

/// The transcript rules (design note 39): spec/transcript-cases.json, the table Web and
/// Android run too, through `OpenAILiveSession` with GPT-Live events and level ticks.
/// No network, no audio.
final class TranscriptTests: XCTestCase {
    private func table() throws -> [String: Any] {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("spec/transcript-cases.json")
        guard let data = try? Data(contentsOf: url) else {
            throw XCTSkip("spec/transcript-cases.json not readable at \(url.path) (device run?)")
        }
        return try JSONSerialization.jsonObject(with: data) as! [String: Any]
    }

    /// An update as the table writes it: only the keys that are set.
    private func dict(_ u: TranscriptUpdate?) -> NSDictionary? {
        guard let u else { return nil }
        var d: [String: Any] = ["role": u.role.rawValue, "text": u.text, "final": u.final, "turnId": u.turnId]
        if u.truncated { d["truncated"] = true }
        if let s = u.startMs { d["startMs"] = s }
        if let e = u.endMs { d["endMs"] = e }
        return d as NSDictionary
    }

    func testTheAssemblerFollowsTheSharedTable() throws {
        let root = try table()
        let k = root["constants"] as! [String: Double]
        XCTAssertEqual(k["userSilenceMs"], TranscriptAssembler.userSilenceMs)
        XCTAssertEqual(k["userCloseChars"], Double(TranscriptAssembler.userCloseChars))
        XCTAssertEqual(k["revealCharsPerSecond"], TranscriptAssembler.revealCharsPerSecond)
        XCTAssertEqual(k["cutGraceMs"], TranscriptAssembler.cutGraceMs)
        XCTAssertEqual(k["segmentDelayMs"], TranscriptAssembler.segmentDelayMs)
        XCTAssertEqual(k["newUtteranceGapMs"], TranscriptAssembler.newUtteranceGapMs)
        XCTAssertEqual(k["audibleLevel"], TranscriptAssembler.audibleLevel)
        let cases = root["cases"] as! [[String: Any]]
        XCTAssertGreaterThanOrEqual(cases.count, 12)
        for c in cases {
            let name = c["name"] as! String
            let s = OpenAILiveSession(syncToAudio: c["syncToAudio"] as! Bool)
            var lastUser: TranscriptUpdate?
            var lastAssistant: TranscriptUpdate?
            var finals = 0
            var finalIds = Set<String>()
            var shown: [String: String] = [:]
            s.onTranscript = { u in
                XCTAssertFalse(finalIds.contains(u.turnId), "\(name): update after \(u.turnId)'s final")
                if !u.truncated, !u.final {
                    XCTAssertTrue(u.text.hasPrefix(shown[u.turnId] ?? ""), "\(name): \(u.turnId) shrank")
                }
                shown[u.turnId] = u.text
                if u.final {
                    finalIds.insert(u.turnId)
                    finals += 1
                }
                if u.role == .user { lastUser = u } else { lastAssistant = u }
            }
            s.connecting()
            for (i, step) in (c["steps"] as! [[String: Any]]).enumerated() {
                let at = "\(name), step \(i + 1)"
                let t = (step["t"] as! NSNumber).doubleValue
                if let ev = step["event"] {
                    let text = String(decoding: try JSONSerialization.data(withJSONObject: ev), as: UTF8.self)
                    s.handle(text, now: t)
                } else if step["reconnect"] != nil {
                    s.connecting()
                } else {
                    let level = (step["level"] as! NSNumber).doubleValue
                    let repeats = (step["repeat"] as? NSNumber)?.intValue ?? 1
                    for n in 0..<repeats { s.tick(level: level, now: t + Double(n) * 33) }
                }
                XCTAssertEqual(dict(lastUser), step["user"] as? NSDictionary, "\(at): user")
                XCTAssertEqual(dict(lastAssistant), step["assistant"] as? NSDictionary, "\(at): assistant")
                XCTAssertEqual(finals, step["finals"] as? Int, "\(at): finals")
            }
        }
    }

    func testSharedVoiceSourceFansOutTranscriptsAndUnsubscribes() {
        final class Fake: VoiceSource {
            var cb: ((TranscriptUpdate) -> Void)?
            func connect() async throws {}
            func disconnect() {}
            func onMetrics(_ cb: @escaping (VoiceMetrics) -> Void) {}
            func onStateChange(_ cb: @escaping (AgentState) -> Void) {}
            func onTranscript(_ cb: @escaping (TranscriptUpdate) -> Void) { self.cb = cb }
            var supportsTranscript: Bool { true }
            var transcriptTiming: TranscriptTiming { .segments }
        }
        let fake = Fake()
        let shared = SharedVoiceSource.of(fake)
        var a: [String] = []
        var b: [String] = []
        let offA = shared.listenTranscript { a.append($0.text) }
        shared.listenTranscript { b.append($0.text) }
        XCTAssertTrue(shared.supportsTranscript)
        XCTAssertEqual(shared.transcriptTiming, .segments)
        fake.cb?(TranscriptUpdate(role: .user, text: "Merhaba", final: false, turnId: "u1"))
        offA()
        fake.cb?(TranscriptUpdate(role: .user, text: "Merhaba!", final: true, turnId: "u1"))
        XCTAssertEqual(a, ["Merhaba"])
        XCTAssertEqual(b, ["Merhaba", "Merhaba!"])
    }
}
