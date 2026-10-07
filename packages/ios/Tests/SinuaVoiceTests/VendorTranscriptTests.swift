import XCTest

@testable import SinuaVoice

/// T1b (design note 39): each vendor session feeds its transcript. The rules themselves are
/// the shared tables in TranscriptTests; these check the wiring, with the clock passed in.
/// No network, no audio.
final class VendorTranscriptTests: XCTestCase {
    private func json(_ obj: [String: Any]) -> String {
        String(decoding: try! JSONSerialization.data(withJSONObject: obj), as: UTF8.self)
    }

    /// `n` samples of 16-bit PCM, base64.
    private func pcm(_ n: Int) -> String {
        Data(count: n * 2).base64EncodedString()
    }

    private let line = "Bir zamanlar bir sincap yaşarmış."

    func testElevenLabsRevealsTheAlignmentAndCutsAtTheHeardCharacter() {
        let s = ElevenLabsSession()
        var got: [TranscriptUpdate] = []
        s.onTranscript = { got.append($0) }
        s.connecting(now: 0)
        _ = s.graphStarted(output: .init(codec: .pcm, rate: 16000), now: 0)
        _ = s.handle(
            json(["type": "user_transcript", "user_transcription_event": ["user_transcript": "Bir hikaye anlat"]]),
            playback: .drained, now: 0.1)
        XCTAssertEqual(got.last, TranscriptUpdate(role: .user, text: "Bir hikaye anlat", final: true, turnId: "u1"))
        let chars = line.map(String.init)
        let alignment: [String: Any] = [
            "chars": chars, "char_start_times_ms": chars.indices.map { $0 * 60 },
            "char_durations_ms": chars.map { _ in 60 },
        ]
        _ = s.handle(
            json([
                "type": "audio", "audio_event": ["audio_base_64": pcm(32000), "event_id": 2, "alignment": alignment],
            ]),
            playback: .drained, now: 1)
        // Audible from 1.0 s (the session says speaking a tick later); by 1.6 s, ~9 of 60 ms each.
        var t = 1.0
        while t <= 1.6 {
            s.tick(playback: .audible, level: 0.5, now: t)
            t += 1.0 / 30
        }
        let mid = got.last { $0.role == .assistant }!
        XCTAssertFalse(mid.final)
        XCTAssertTrue(line.hasPrefix(mid.text) && mid.text.count >= 7 && mid.text.count <= 11, mid.text)
        _ = s.handle(
            json(["type": "interruption", "interruption_event": ["event_id": 3]]), playback: .audible, now: 1.65)
        let cut = got.last!
        XCTAssertTrue(cut.final && cut.truncated)
        XCTAssertTrue(line.hasPrefix(cut.text) && cut.text.count < line.count, cut.text)
    }

    func testGeminiAsksForBothTranscriptionsAndPacesTheReplyOverItsAudio() {
        let s = GeminiLiveSession(model: "m")
        var got: [TranscriptUpdate] = []
        s.onTranscript = { got.append($0) }
        let setup =
            (try! JSONSerialization.jsonObject(with: Data(s.setupMessage().utf8)) as! [String: Any])["setup"]
            as! [String: Any]
        XCTAssertNotNil(setup["inputAudioTranscription"])
        XCTAssertNotNil(setup["outputAudioTranscription"])
        s.connecting()
        _ = s.handle(json(["setupComplete": [:]]), playback: .drained, now: 0)
        _ = s.handle(
            json(["serverContent": ["inputTranscription": ["text": "Bir hikaye"]]]), playback: .drained, now: 0.1)
        _ = s.handle(json(["serverContent": ["inputTranscription": ["text": " anlat"]]]), playback: .drained, now: 0.2)
        XCTAssertEqual(got.last, TranscriptUpdate(role: .user, text: "Bir hikaye anlat", final: false, turnId: "u1"))
        // 3 s of audio for the line (24 kHz), then its text.
        _ = s.handle(
            json([
                "serverContent": [
                    "modelTurn": ["parts": [["inlineData": ["data": pcm(72000), "mimeType": "audio/pcm;rate=24000"]]]]
                ]
            ]),
            playback: .queued, now: 1)
        _ = s.handle(json(["serverContent": ["outputTranscription": ["text": line]]]), playback: .queued, now: 1)
        XCTAssertEqual(
            got.filter { $0.role == .user && $0.final }.count, 1, "a 12+ character reply ends the user's turn")
        var t = 1.0
        while t <= 2.5 {
            s.tick(playback: .audible, level: 0.5, now: t)
            t += 1.0 / 30
        }
        // Half the audio played: about half the line.
        let mid = got.last { $0.role == .assistant }!
        XCTAssertTrue(line.hasPrefix(mid.text) && abs(mid.text.count - line.count / 2) <= 2, mid.text)
        _ = s.handle(json(["serverContent": ["interrupted": true]]), playback: .audible, now: 2.55)
        XCTAssertTrue(got.last!.final && got.last!.truncated)
        XCTAssertTrue(line.hasPrefix(got.last!.text) && got.last!.text.count < line.count)
    }

    func testRealtimeTurnsInputTranscriptionOnOnceAndCutsOnABargeIn() {
        let s = OpenAIRealtimeSession()
        var sent: [String] = []
        s.onSend = { sent.append($0) }
        var got: [TranscriptUpdate] = []
        s.onTranscript = { got.append($0) }
        s.connecting()
        s.handle(
            json(["type": "session.created", "session": ["audio": ["input": ["transcription": NSNull()]]]]), now: 0)
        XCTAssertEqual(sent.count, 1)
        let update = try! JSONSerialization.jsonObject(with: Data(sent[0].utf8)) as! NSDictionary
        XCTAssertEqual(
            update,
            [
                "type": "session.update",
                "session": [
                    "type": "realtime", "audio": ["input": ["transcription": ["model": "gpt-4o-mini-transcribe"]]],
                ],
            ] as NSDictionary)
        s.handle(json(["type": "conversation.item.input_audio_transcription.delta", "delta": "Bir hikaye"]), now: 0.1)
        s.handle(
            json(["type": "conversation.item.input_audio_transcription.completed", "transcript": "Bir hikaye anlat."]),
            now: 0.5)
        XCTAssertEqual(got.last, TranscriptUpdate(role: .user, text: "Bir hikaye anlat.", final: true, turnId: "u1"))
        s.handle(json(["type": "response.created"]), now: 1)
        s.handle(json(["type": "response.output_audio_transcript.delta", "delta": line]), now: 1)
        s.handle(json(["type": "output_audio_buffer.started"]), now: 1)
        var t = 1.0
        while t <= 1.5 {
            s.tick(level: 0.5, now: t)
            t += 1.0 / 30
        }
        s.handle(json(["type": "input_audio_buffer.speech_started"]), now: 1.55)
        let cut = got.last!
        XCTAssertTrue(cut.final && cut.truncated)
        XCTAssertTrue(line.hasPrefix(cut.text) && !cut.text.isEmpty && cut.text.count < line.count, cut.text)
        // A session with its own transcription is left alone; a new call asks again.
        s.connecting()
        s.handle(
            json([
                "type": "session.created", "session": ["audio": ["input": ["transcription": ["model": "whisper-1"]]]],
            ]))
        XCTAssertEqual(sent.count, 1)
        let off = OpenAIRealtimeSession(transcribeUser: nil)
        var offSent = 0
        off.onSend = { _ in offSent += 1 }
        off.onTranscript = { _ in }
        off.connecting()
        off.handle(json(["type": "session.created", "session": [:]]))
        XCTAssertEqual(offSent, 0)
    }

    func testLiveKitPassesSegmentsThroughAndEndsTheAgentTurnWhenItStops() {
        let t = LiveKitAgentTracker()
        var got: [TranscriptUpdate] = []
        t.onTranscript = { got.append($0) }
        t.start()
        t.participantSeen(identity: "agent", isAgentKind: true, attributes: ["lk.agent.state": "listening"])
        t.transcription(
            identity: "me", isAgentKind: false, attributes: [:], local: true, key: "SG_1", text: "Hava nasıl?",
            final: true, now: 0)
        XCTAssertEqual(got.last, TranscriptUpdate(role: .user, text: "Hava nasıl?", final: true, turnId: "u1"))
        t.transcription(
            identity: "guest", isAgentKind: false, attributes: [:], local: false, key: "SG_9", text: "Selam",
            final: true, now: 1)
        XCTAssertEqual(got.count, 1, "a non-agent participant is not the assistant")
        t.attributesChanged(
            identity: "agent", isAgentKind: true, attributes: ["lk.agent.state": "speaking"],
            changed: ["lk.agent.state": "speaking"], localSpeaking: false)
        t.transcription(
            identity: "agent", isAgentKind: true, attributes: [:], local: false, key: "SG_2", text: "Güneşli ve ılık.",
            final: false, now: 2)
        XCTAssertEqual(
            got.last, TranscriptUpdate(role: .assistant, text: "Güneşli ve ılık.", final: false, turnId: "a1"))
        t.attributesChanged(
            identity: "agent", isAgentKind: true, attributes: ["lk.agent.state": "listening"],
            changed: ["lk.agent.state": "listening"], localSpeaking: false)
        XCTAssertEqual(
            got.last, TranscriptUpdate(role: .assistant, text: "Güneşli ve ılık.", final: true, turnId: "a1"))
    }
}
