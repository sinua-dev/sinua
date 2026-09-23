import XCTest

@testable import SinuaVoice

/// The SDK-free core of the native ElevenLabs source: `Pcm` μ-law / format
/// parsing (pcm.ts) and `ElevenLabsSession` (ElevenLabsVoiceSource.ts's
/// protocol + state machine), with an injected clock.
final class ElevenLabsCoreTests: XCTestCase {
    final class Box {
        var states: [AgentState] = []
        var interrupts = 0
    }

    private func make() -> (ElevenLabsSession, Box) {
        let s = ElevenLabsSession()
        let box = Box()
        s.onState = { box.states.append($0) }
        s.onInterrupt = { box.interrupts += 1 }
        return (s, box)
    }

    private func json(_ s: String) -> [String: Any] {
        (try? JSONSerialization.jsonObject(with: Data(s.utf8))) as? [String: Any] ?? [:]
    }

    private func audio(id: Int, _ samples: [Float] = [Float](repeating: 0.25, count: 160)) -> String {
        #"{"type":"audio","audio_event":{"audio_base_64":"\#(Pcm.floatToPcm16(samples).base64EncodedString())","event_id":\#(id)}}"#
    }

    private func audio(id: Int, isFinal: Bool) -> String {
        let b64 = Pcm.floatToPcm16([Float](repeating: 0.25, count: 160)).base64EncodedString()
        return #"{"type":"audio","audio_event":{"audio_base_64":"\#(b64)","event_id":\#(id),"is_final":\#(isFinal)}}"#
    }

    private let complete = #"{"type":"agent_response_complete","agent_response_complete_event":{"event_id":9}}"#
    private let agentText =
        #"{"type":"agent_response","agent_response_event":{"agent_response":"Hello.","event_id":9}}"#

    private let metadata =
        #"{"type":"conversation_initiation_metadata","conversation_initiation_metadata_event":{"conversation_id":"c1","agent_output_audio_format":"pcm_44100","user_input_audio_format":"pcm_16000"}}"#

    /// Connected and the graph up at 44.1 kHz (the usual path).
    private func started(_ s: ElevenLabsSession, now: TimeInterval = 0) {
        s.connecting(now: now)
        _ = s.handle(metadata, playback: .drained, now: now)
        _ = s.graphStarted(output: .init(codec: .pcm, rate: 44100), now: now)
    }

    // MARK: - Pcm

    func testParseAudioFormatLikeWeb() {
        XCTAssertEqual(Pcm.parseAudioFormat("pcm_44100"), .init(codec: .pcm, rate: 44100))
        XCTAssertEqual(Pcm.parseAudioFormat(" ULAW_8000 "), .init(codec: .ulaw, rate: 8000))
        XCTAssertEqual(Pcm.parseAudioFormat("mp3_44100"), .init(codec: .pcm, rate: 16000), "unknown -> fallback")
        XCTAssertEqual(Pcm.parseAudioFormat(nil), .init(codec: .pcm, rate: 16000))
        XCTAssertEqual(Pcm.parseAudioFormat("pcm_16000_x"), .init(codec: .pcm, rate: 16000))
    }

    func testUlawDecodeMatchesTheG711Table() {
        // 0xFF / 0x7F are +0 / -0; 0x80 is the most positive code, 0x00 the most negative.
        let out = Pcm.ulawToFloat(Data([0xFF, 0x7F, 0x80, 0x00, 0xF0]))
        XCTAssertEqual(out[0], 0)
        XCTAssertEqual(out[1], 0)
        XCTAssertEqual(out[2], Float(16764 + (15 << 10)) / 32768)
        XCTAssertEqual(out[3], -Float(16764 + (15 << 10)) / 32768)
        XCTAssertEqual(out[4], Float(0 + (15 << 3)) / 32768, "exponent 0, mantissa 15")
    }

    // MARK: - Messages

    func testEndpointInitAndMicMessages() {
        XCTAssertEqual(
            ElevenLabsSession.endpoint(credential: " agent_abc ")?.absoluteString,
            "wss://api.elevenlabs.io/v1/convai/conversation?agent_id=agent_abc")
        XCTAssertEqual(
            ElevenLabsSession.endpoint(credential: "wss://signed.example/x?token=1")?.absoluteString,
            "wss://signed.example/x?token=1", "a signed URL is used as-is")
        XCTAssertEqual(json(ElevenLabsSession.initMessage())["type"] as? String, "conversation_initiation_client_data")
        XCTAssertNil(json(ElevenLabsSession.initMessage())["conversation_config_override"])
        let withOv = json(ElevenLabsSession.initMessage(overrides: ["agent": ["language": "tr"]]))
        XCTAssertEqual(
            ((withOv["conversation_config_override"] as? [String: Any])?["agent"] as? [String: String])?["language"],
            "tr")
        XCTAssertEqual(
            json(ElevenLabsSession.micMessage([0.5, -0.5] as [Float]))["user_audio_chunk"] as? String,
            Pcm.floatToPcm16([0.5, -0.5] as [Float]).base64EncodedString())
    }

    func testMetadataAndPingPong() {
        let (s, _) = make()
        s.connecting(now: 0)
        XCTAssertEqual(
            s.handle(metadata, playback: .drained, now: 0),
            [.metadata(input: .init(codec: .pcm, rate: 16000), output: .init(codec: .pcm, rate: 44100))])
        guard
            case .send(let pong)? = s.handle(
                #"{"type":"ping","ping_event":{"event_id":7,"ping_ms":40}}"#, playback: .drained, now: 0
            ).first
        else { return XCTFail("expected a pong") }
        XCTAssertEqual(json(pong)["type"] as? String, "pong")
        XCTAssertEqual(json(pong)["event_id"] as? Int, 7)
    }

    // MARK: - States

    func testPendingAudioIsFlushedWhenTheGraphStarts() {
        let (s, box) = make()
        s.connecting(now: 0)
        _ = s.handle(metadata, playback: .drained, now: 0)
        XCTAssertEqual(s.handle(audio(id: 1), playback: .drained, now: 0), [], "graph not up yet: held")
        let flushed = s.graphStarted(output: .init(codec: .pcm, rate: 44100), now: 0)
        guard case .enqueue(let samples, let rate)? = flushed.first else { return XCTFail("expected the held chunk") }
        XCTAssertEqual(samples.count, 160)
        XCTAssertEqual(rate, 44100)
        XCTAssertEqual(box.states, [.initializing, .thinking], "the greeting is on its way; not listening first")
    }

    func testTurnFollowsVadTranscriptAndPlayback() {
        let (s, box) = make()
        started(s)
        XCTAssertEqual(box.states, [.initializing, .listening])
        _ = s.handle(#"{"type":"vad_score","vad_score_event":{"vad_score":0.9}}"#, playback: .drained, now: 1)
        _ = s.handle(#"{"type":"vad_score","vad_score_event":{"vad_score":0.1}}"#, playback: .drained, now: 2)
        XCTAssertEqual(s.state, .thinking, "the user stopped: the agent is working")
        _ = s.handle(audio(id: 2), playback: .drained, now: 2.5)
        s.tick(playback: .queued, now: 2.6)
        XCTAssertEqual(s.state, .thinking)
        s.tick(playback: .audible, now: 2.7)
        XCTAssertEqual(s.state, .speaking)
        _ = s.handle(complete, playback: .audible, now: 2.8)
        XCTAssertEqual(s.state, .speaking, "complete is the server's end; the audio still plays")
        s.tick(playback: .drained, now: 3)
        XCTAssertEqual(s.state, .listening, "complete + drained is the end of the agent's turn")
        _ = s.handle(
            #"{"type":"user_transcript","user_transcription_event":{"user_transcript":"hi"}}"#, playback: .drained,
            now: 4)
        XCTAssertEqual(s.state, .thinking)
        s.tick(playback: .drained, now: 7.9)
        XCTAssertEqual(s.state, .thinking)
        s.tick(playback: .drained, now: 8.1)
        XCTAssertEqual(s.state, .listening, "4 s without audio: back to listening")
        XCTAssertEqual(box.interrupts, 0)
    }

    func testVadWhileTheAgentSpeaksDoesNotStealTheState() {
        let (s, _) = make()
        started(s)
        _ = s.handle(audio(id: 1), playback: .drained, now: 0)
        s.tick(playback: .audible, now: 0.2)
        _ = s.handle(#"{"type":"vad_score","vad_score_event":{"vad_score":0.95}}"#, playback: .audible, now: 0.3)
        XCTAssertEqual(s.state, .speaking)
    }

    func testInterruptionDropsLateChunksAndFlashesOnlyWithOutput() {
        let (s, box) = make()
        started(s)
        _ = s.handle(audio(id: 3), playback: .drained, now: 0)
        s.tick(playback: .audible, now: 0.1)
        XCTAssertEqual(
            s.handle(#"{"type":"interruption","interruption_event":{"event_id":5}}"#, playback: .audible, now: 0.2),
            [.clearPlayback(fade: true)])
        XCTAssertEqual(box.interrupts, 1)
        XCTAssertEqual(s.state, .listening)
        XCTAssertEqual(s.handle(audio(id: 4), playback: .drained, now: 0.3), [], "a late chunk of the cut-off response")
        XCTAssertEqual(s.handle(audio(id: 5), playback: .drained, now: 0.3).count, 1, "the new response plays")
        _ = s.handle(#"{"type":"interruption","interruption_event":{"event_id":9}}"#, playback: .drained, now: 1)
        XCTAssertEqual(box.interrupts, 1, "not speaking and the graph drained: nothing to cut, no barge-in")
    }

    func testInterruptionWithNothingToCut() {
        let (s, box) = make()
        started(s)
        _ = s.handle(#"{"type":"interruption","interruption_event":{"event_id":2}}"#, playback: .drained, now: 0)
        XCTAssertEqual(box.interrupts, 0)
    }

    func testUlawOutputIsDecodedAtItsRate() {
        let (s, _) = make()
        s.connecting(now: 0)
        _ = s.graphStarted(output: .init(codec: .ulaw, rate: 8000), now: 0)
        let b64 = Data([0x80, 0x00]).base64EncodedString()
        let acts = s.handle(
            #"{"type":"audio","audio_event":{"audio_base_64":"\#(b64)","event_id":1}}"#, playback: .drained, now: 0)
        XCTAssertEqual(acts, [.enqueue(Pcm.ulawToFloat(Data([0x80, 0x00])), rate: 8000)])
    }

    func testMalformedAndIrrelevantFrames() {
        let (s, box) = make()
        started(s)
        XCTAssertEqual(s.handle("nope", playback: .drained, now: 0), [])
        XCTAssertEqual(
            s.handle(
                #"{"type":"agent_response","agent_response_event":{"agent_response":"hi"}}"#, playback: .drained, now: 0
            ), [])
        XCTAssertEqual(s.handle(#"{"type":"agent_response_complete"}"#, playback: .drained, now: 0), [])
        XCTAssertEqual(box.states.last, .listening)
        s.stopped(now: 1)
        XCTAssertEqual(s.state, .idle)
    }

    // MARK: - Turn end: agent_response_complete / is_final (ElevenLabs Agents WebSocket reference)

    func testDrainedMidReplyIsAStallNotTheEndOfTheTurn() {
        let (s, _) = make()
        started(s)
        _ = s.handle(audio(id: 1), playback: .drained, now: 1)
        s.tick(playback: .audible, now: 1.2)
        XCTAssertEqual(s.state, .speaking)
        s.tick(playback: .drained, now: 1.5)
        XCTAssertEqual(s.state, .thinking, "no complete yet: the buffer ran dry, more audio is coming")
        _ = s.handle(audio(id: 1), playback: .drained, now: 1.7)
        s.tick(playback: .audible, now: 1.9)
        XCTAssertEqual(s.state, .speaking)
        _ = s.handle(complete, playback: .audible, now: 2)
        s.tick(playback: .drained, now: 2.3)
        XCTAssertEqual(s.state, .listening)
    }

    func testAFinalChunkEndsTheReplyLikeComplete() {
        let (s, _) = make()
        started(s)
        _ = s.handle(audio(id: 1, isFinal: false), playback: .drained, now: 1)
        _ = s.handle(audio(id: 1, isFinal: true), playback: .queued, now: 1.1)
        s.tick(playback: .audible, now: 1.3)
        s.tick(playback: .drained, now: 1.8)
        XCTAssertEqual(s.state, .listening)
    }

    func testAReplyWithoutAudioEndsThinkingAtOnce() {
        let (s, _) = make()
        started(s)
        _ = s.handle(
            #"{"type":"user_transcript","user_transcription_event":{"user_transcript":"hi"}}"#, playback: .drained,
            now: 1)
        _ = s.handle(agentText, playback: .drained, now: 1.5)
        XCTAssertEqual(s.state, .thinking)
        _ = s.handle(complete, playback: .drained, now: 1.6)
        XCTAssertEqual(s.state, .listening, "text-only reply: no 4 s wait")
    }

    func testAStalledReplyGivesUpAfterTenSeconds() {
        let (s, _) = make()
        started(s)
        _ = s.handle(audio(id: 1), playback: .drained, now: 1)
        s.tick(playback: .audible, now: 1.2)
        s.tick(playback: .drained, now: 1.5)
        XCTAssertEqual(s.state, .thinking)
        s.tick(playback: .drained, now: 6)
        XCTAssertEqual(s.state, .thinking, "the 4 s flicker guard doesn't apply mid-reply")
        s.tick(playback: .drained, now: 11.1)
        XCTAssertEqual(s.state, .listening, "no audio for 10 s and no complete: give up")
    }

    func testChunksAfterCompleteDoNotReopenTheReply() {
        let (s, _) = make()
        started(s)
        _ = s.handle(audio(id: 1), playback: .drained, now: 1)
        _ = s.handle(complete, playback: .queued, now: 1.05)
        _ = s.handle(audio(id: 1), playback: .queued, now: 1.1)  // a trailing chunk after complete
        s.tick(playback: .audible, now: 1.3)
        s.tick(playback: .drained, now: 1.9)
        XCTAssertEqual(s.state, .listening)
        // The user's next turn opens a new reply.
        _ = s.handle(
            #"{"type":"user_transcript","user_transcription_event":{"user_transcript":"and?"}}"#, playback: .drained,
            now: 2)
        _ = s.handle(audio(id: 2), playback: .drained, now: 2.5)
        s.tick(playback: .audible, now: 2.7)
        s.tick(playback: .drained, now: 3)
        XCTAssertEqual(s.state, .thinking, "a new reply is open again until its complete")
    }

    func testInterruptionClosesTheReply() {
        let (s, box) = make()
        started(s)
        _ = s.handle(audio(id: 1), playback: .drained, now: 1)
        s.tick(playback: .audible, now: 1.2)
        let actions = s.handle(
            #"{"type":"interruption","interruption_event":{"event_id":3}}"#, playback: .audible, now: 1.4)
        XCTAssertEqual(actions, [.clearPlayback(fade: true)])
        XCTAssertEqual(s.state, .listening)
        XCTAssertEqual(box.interrupts, 1)
        s.tick(playback: .drained, now: 1.6)
        XCTAssertEqual(s.state, .listening)
    }
}
