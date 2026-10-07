import XCTest

@testable import SinuaVoice

/// The transcript rules (design note 39): spec/transcript-cases.json, the table Web and
/// Android run too, through `OpenAILiveSession` with GPT-Live events and level ticks.
/// No network, no audio.
final class TranscriptTests: XCTestCase {
    private func table(_ file: String = "transcript-cases.json") throws -> [String: Any] {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("spec/\(file)")
        guard let data = try? Data(contentsOf: url) else {
            throw XCTSkip("spec/\(file) not readable at \(url.path) (device run?)")
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

    /// T1b: the vendor-free half on its own (chars, none paced over received audio, explicit user
    /// end, keyed segments): spec/transcript-assembler-cases.json.
    func testTheAssemblerFollowsTheVendorFreeTable() throws {
        let root = try table("transcript-assembler-cases.json")
        let k = root["constants"] as! [String: Double]
        XCTAssertEqual(k["rateMinAudioMs"], TranscriptAssembler.rateMinAudioMs)
        let cases = root["cases"] as! [[String: Any]]
        XCTAssertGreaterThanOrEqual(cases.count, 9)
        for c in cases {
            let name = c["name"] as! String
            let a = TranscriptAssembler(
                timing: TranscriptTiming(rawValue: c["timing"] as! String)!, sync: c["syncToAudio"] as! Bool,
                explicitUserEnd: c["explicitUserEnd"] as! Bool)
            var lastUser: TranscriptUpdate?
            var lastAssistant: TranscriptUpdate?
            var finals = 0
            a.onUpdate = { u in
                if u.final { finals += 1 }
                if u.role == .user { lastUser = u } else { lastAssistant = u }
            }
            for (i, step) in (c["steps"] as! [[String: Any]]).enumerated() {
                let t = (step["t"] as! NSNumber).doubleValue
                let num = { (key: String) in (step[key] as? NSNumber)?.doubleValue }
                let text = step["text"] as? String
                switch step["op"] as! String {
                case "tick":
                    let speaking = step["speaking"] as! Bool
                    for n in 0..<(step["repeat"] as! Int) {
                        a.tick(now: t + Double(n) * 33, level: num("level")!, speaking: speaking)
                    }
                case "user": a.userDelta(text!, now: t)
                case "assistant": a.assistantDelta(text!, now: t, startMs: num("startMs"), endMs: num("endMs"))
                case "userDone": a.userDone(text, now: t)
                case "audio": a.assistantAudio(num("ms")!)
                case "segment":
                    a.segment(
                        TranscriptUpdate.Role(rawValue: step["role"] as! String)!, key: step["key"] as! String,
                        text: text!, final: step["final"] as! Bool, now: t)
                case "hold": a.hold()
                case "cut": a.cut()
                case "ended": a.speakingEnded()
                case "stop": a.stop()
                case let op: XCTFail("unknown op \(op)")
                }
                let at = "\(name), step \(i + 1)"
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
