import XCTest

@testable import SinuaVoice

/// A `PcmAudioDevice` whose clock the test drives; records what was scheduled.
final class FakePcmDevice: PcmAudioDevice {
    var playedFrames: Int64 = 0
    var scheduled: [(frame: Int64, count: Int)] = []
    var resets = 0
    var started = false
    var onCapture: (@Sendable ([Float]) -> Void)?
    var onClockReset: (() -> Void)?

    func start(inputRate _: Int, outputRate _: Int, onCapture: @escaping @Sendable ([Float]) -> Void) throws {
        started = true
        self.onCapture = onCapture
    }

    func schedule(_ samples: [Float], atFrame frame: Int64) { scheduled.append((frame, samples.count)) }

    func resetPlayback(fade _: Bool) {
        resets += 1
        playedFrames = 0
        scheduled.removeAll()
    }

    func stop() { started = false }
}

/// The SDK-free core of the native Gemini Live source: `Pcm` (pcm.ts), the
/// playback timeline (PcmAudioGraph.ts), and `GeminiLiveSession`
/// (PCMStreamVoiceSource.ts's protocol + state machine).
final class GeminiLiveCoreTests: XCTestCase {
    // MARK: - Pcm

    func testPcm16RoundTripClampsAndRoundsLikeWeb() {
        let data = Pcm.floatToPcm16([0, 0.5, -0.5, 1, -1, 2, -2, 1e-5] as [Float])
        XCTAssertEqual(data.count, 16)
        let back = Pcm.pcm16ToFloat(data)
        // float32ToPcm16: clamp, round(s*32768), clamp to Int16 -> 1.0 saturates at 32767.
        XCTAssertEqual(back, [0, 0.5, -0.5, 32767 / 32768, -1, 32767 / 32768, -1, 0])
        XCTAssertEqual(Array(Pcm.floatToPcm16([0.5] as [Float])), [0x00, 0x40])  // little-endian 16384
        XCTAssertEqual(Pcm.pcm16ToFloat(Data([0x00, 0x40, 0xFF])), [0.5], "a trailing odd byte is ignored")
        // Exact negative half: JS Math.round(-0.5) is -0 -> 0, not -1.
        XCTAssertEqual(Array(Pcm.floatToPcm16([-1 / 65536] as [Float])), [0x00, 0x00])
    }

    func testParseRate() {
        XCTAssertEqual(Pcm.parseRate("audio/pcm;rate=24000"), 24000)
        XCTAssertEqual(Pcm.parseRate("audio/pcm; RATE=16000"), 16000)
        XCTAssertEqual(Pcm.parseRate("audio/pcm", fallback: 22050), 22050)
        XCTAssertEqual(Pcm.parseRate(nil), 24000)
    }

    func testResampleOnlyWhenRatesDiffer() {
        let s: [Float] = [0, 1, 0, -1]
        XCTAssertEqual(Pcm.resample(s, from: 24000, to: 24000), s)
        XCTAssertEqual(Pcm.resample(s, from: 12000, to: 24000).count, 8)
        XCTAssertEqual(Pcm.resample(s, from: 12000, to: 24000)[1], 0.5, accuracy: 1e-6)
    }

    // MARK: - PlaybackTimeline

    func testTimelineLeadCursorAndStates() {
        let t = PlaybackTimeline(rate: 24000)  // lead 2400 frames
        XCTAssertEqual(t.state(now: 0), .drained)
        XCTAssertEqual(t.append([Float](repeating: 0.1, count: 1000), now: 100), 2500, "new burst: now + lead")
        XCTAssertEqual(t.append([Float](repeating: 0.2, count: 1000), now: 200), 3500, "gap-free after the cursor")
        XCTAssertEqual(t.state(now: 2499), .queued)
        XCTAssertEqual(t.state(now: 2500), .audible)
        XCTAssertEqual(t.state(now: 4499), .audible)
        XCTAssertEqual(t.state(now: 4500), .drained)
        // A later chunk after draining starts a new burst with a fresh lead.
        XCTAssertEqual(t.append([Float](repeating: 0.3, count: 10), now: 5000), 7400)
        XCTAssertEqual(t.state(now: 7000), .queued)
        t.clear()
        XCTAssertEqual(t.state(now: 7000), .drained)
        XCTAssertEqual(t.cursor, 0)
    }

    func testTimelinePlayedReturnsOnlyWhatHasPlayed() {
        let t = PlaybackTimeline(rate: 24000)
        t.append([Float](repeating: 0.5, count: 100), now: 0)  // frames 2400..<2500
        XCTAssertEqual(t.played(from: 0, to: 2400), [Float](repeating: 0, count: 2400), "lead is silence")
        let p = t.played(from: 2390, to: 2410)
        XCTAssertEqual(p, [Float](repeating: 0, count: 10) + [Float](repeating: 0.5, count: 10))
        XCTAssertEqual(t.played(from: 0, to: 3000, maxCount: 512).count, 512, "only the newest maxCount")
    }

    // MARK: - PcmAudioGraph over a fake device

    func testGraphSchedulesOnTheDeviceClockAndAnalysesPlayedSamplesOnly() throws {
        let dev = FakePcmDevice()
        let g = PcmAudioGraph(device: dev)
        XCTAssertNil(g.read(), "no metrics before start")
        try g.start(inputRate: 16000, outputRate: 24000) { _ in }
        let tone = (0..<4800).map { Float(0.5 * sin(2 * .pi * 440 * Double($0) / 24000)) }
        g.enqueue(tone, rate: 24000)
        XCTAssertEqual(dev.scheduled.first?.frame, 2400)
        XCTAssertEqual(g.playbackState(), .queued)
        XCTAssertEqual(g.read()?.level, 0, "received but not played: silent")
        dev.playedFrames = 2400 + 2000
        XCTAssertEqual(g.playbackState(), .audible)
        XCTAssertGreaterThan(g.read()!.level, 0.05, "played: the analyser sees it")
        dev.playedFrames = 2400 + 4800
        XCTAssertEqual(g.playbackState(), .drained)
        g.clearPlayback(fade: true)
        XCTAssertEqual(dev.resets, 1)
        XCTAssertEqual(g.playbackState(), .drained)
        g.enqueue(tone, rate: 24000)
        XCTAssertEqual(g.playbackState(), .queued)
        dev.playedFrames = 0
        dev.onClockReset?()  // e.g. the engine restarted after a route change: queued audio is gone
        XCTAssertEqual(g.playbackState(), .drained)
        g.stop()
        XCTAssertFalse(dev.started)
    }

    // MARK: - GeminiLiveSession

    final class StatesBox {
        var states: [AgentState] = []
        var interrupts = 0
    }

    private func make() -> (GeminiLiveSession, StatesBox) {
        let s = GeminiLiveSession(model: "gemini-3.8-live", instructions: "Be brief.")
        let box = StatesBox()
        s.onState = { box.states.append($0) }
        s.onInterrupt = { box.interrupts += 1 }
        return (s, box)
    }

    private func json(_ s: String) -> [String: Any] {
        (try? JSONSerialization.jsonObject(with: Data(s.utf8))) as? [String: Any] ?? [:]
    }

    private func audio(_ n: Int = 480) -> String {
        let b64 = Pcm.floatToPcm16([Float](repeating: 0.25, count: n)).base64EncodedString()
        return
            #"{"serverContent":{"modelTurn":{"parts":[{"inlineData":{"data":"\#(b64)","mimeType":"audio/pcm;rate=24000"}}]}}}"#
    }

    func testEndpointRoutesCredentialsToHeaders() {
        let token = GeminiLiveSession.endpoint(credential: " auth_tokens/abc ")
        XCTAssertEqual(
            token.url.absoluteString,
            "wss://generativelanguage.googleapis.com/ws/google.ai.generativelanguage.v1beta.GenerativeService.BidiGenerateContentConstrained"
        )
        XCTAssertEqual(token.headers, ["Authorization": "Token auth_tokens/abc"])
        let key = GeminiLiveSession.endpoint(credential: "AIzaKEY")
        XCTAssertTrue(key.url.absoluteString.hasSuffix(".BidiGenerateContent"))
        XCTAssertEqual(key.headers, ["x-goog-api-key": "AIzaKEY"])
        XCTAssertNil(key.url.query, "no secret in the URL")
    }

    func testSetupAndMicMessagesMatchWebShape() {
        let (s, _) = make()
        let setup = json(s.setupMessage())["setup"] as! [String: Any]
        XCTAssertEqual(setup["model"] as? String, "models/gemini-3.8-live")
        XCTAssertEqual((setup["generationConfig"] as? [String: Any])?["responseModalities"] as? [String], ["AUDIO"])
        XCTAssertNotNil(setup["inputAudioTranscription"])
        XCTAssertEqual((setup["sessionResumption"] as? [String: Any])?.count, 0)
        XCTAssertNotNil((setup["contextWindowCompression"] as? [String: Any])?["slidingWindow"])
        let parts = (setup["systemInstruction"] as? [String: Any])?["parts"] as? [[String: String]]
        XCTAssertEqual(parts?.first?["text"], "Be brief.")
        XCTAssertEqual(
            json(GeminiLiveSession(model: "models/x").setupMessage())["setup"].flatMap {
                ($0 as? [String: Any])?["model"] as? String
            }, "models/x")

        let mic = json(GeminiLiveSession.micMessage([0.5, -0.5] as [Float]))
        let audio = (mic["realtimeInput"] as? [String: Any])?["audio"] as? [String: String]
        XCTAssertEqual(audio?["mimeType"], "audio/pcm;rate=16000")
        XCTAssertEqual(audio?["data"], Pcm.floatToPcm16([0.5, -0.5] as [Float]).base64EncodedString())
    }

    func testTurnFollowsThePlaybackTimeline() {
        let (s, box) = make()
        s.connecting()
        XCTAssertEqual(s.handle(#"{"setupComplete":{}}"#, playback: .drained), [.setupComplete])
        s.tick(playback: .drained)
        let acts = s.handle(audio(), playback: .drained)
        guard case .enqueue(let samples, let rate)? = acts.first else { return XCTFail("expected enqueue") }
        XCTAssertEqual(samples.count, 480)
        XCTAssertEqual(rate, 24000)
        s.tick(playback: .queued)
        XCTAssertEqual(s.state, .thinking, "received, not audible")
        s.tick(playback: .audible)
        XCTAssertEqual(s.state, .speaking)
        s.tick(playback: .drained)
        XCTAssertEqual(s.state, .thinking, "buffer starved mid-generation")
        _ = s.handle(audio(), playback: .drained)
        s.tick(playback: .audible)
        _ = s.handle(#"{"serverContent":{"turnComplete":true}}"#, playback: .audible)
        XCTAssertEqual(s.state, .speaking, "turnComplete doesn't cut what's still playing")
        s.tick(playback: .drained)
        XCTAssertEqual(s.state, .listening)
        XCTAssertEqual(box.states, [.initializing, .listening, .thinking, .speaking, .thinking, .speaking, .listening])
        XCTAssertEqual(box.interrupts, 0)
    }

    func testInterruptedCutsPlaybackAndFlashesOnlyIfThereWasOutput() {
        let (s, box) = make()
        s.connecting()
        _ = s.handle(#"{"setupComplete":{}}"#, playback: .drained)
        _ = s.handle(audio(), playback: .drained)
        s.tick(playback: .audible)
        let acts = s.handle(#"{"serverContent":{"interrupted":true}}"#, playback: .audible)
        XCTAssertEqual(acts, [.clearPlayback(fade: true)])
        XCTAssertEqual(box.interrupts, 1)
        XCTAssertEqual(s.state, .listening)
        _ = s.handle(#"{"serverContent":{"interrupted":true}}"#, playback: .drained)
        XCTAssertEqual(box.interrupts, 1, "nothing to cut: no barge-in")
        // Queued (not yet audible) output still counts as output to cut.
        _ = s.handle(audio(), playback: .drained)
        _ = s.handle(#"{"serverContent":{"interrupted":true}}"#, playback: .queued)
        XCTAssertEqual(box.interrupts, 2)
    }

    func testInputTranscriptionMeansListeningUnlessSpeaking() {
        let (s, _) = make()
        s.connecting()
        _ = s.handle(#"{"setupComplete":{}}"#, playback: .drained)
        _ = s.handle(audio(), playback: .drained)
        XCTAssertEqual(s.state, .thinking)
        _ = s.handle(#"{"serverContent":{"inputTranscription":{"text":"hi"}}}"#, playback: .queued)
        XCTAssertEqual(s.state, .listening)
        s.tick(playback: .audible)
        _ = s.handle(#"{"serverContent":{"interimInputTranscription":{"text":"h"}}}"#, playback: .audible)
        XCTAssertEqual(s.state, .speaking)
    }

    func testResumptionHandleGoAwayAndReset() {
        let (s, _) = make()
        s.connecting()
        _ = s.handle(#"{"setupComplete":{}}"#, playback: .drained)
        _ = s.handle(#"{"sessionResumptionUpdate":{"newHandle":"h1","resumable":false}}"#, playback: .drained)
        XCTAssertNil(s.resumptionHandle, "only resumable handles are kept")
        _ = s.handle(#"{"sessionResumptionUpdate":{"newHandle":"h2","resumable":true}}"#, playback: .drained)
        XCTAssertEqual(s.resumptionHandle, "h2")
        XCTAssertEqual(
            s.handle(#"{"goAway":{"timeLeft":"5s"}}"#, playback: .drained), [.reconnect(reason: "goAway (timeLeft 5s)")]
        )
        s.connecting()
        XCTAssertEqual(s.state, .initializing)
        let setup = json(s.setupMessage())["setup"] as! [String: Any]
        XCTAssertEqual(
            (setup["sessionResumption"] as? [String: String])?["handle"], "h2", "the reopened socket resumes")
        s.tick(playback: .audible)
        XCTAssertEqual(s.state, .initializing, "no playback-driven state before setupComplete")
        s.reset()
        XCTAssertNil(s.resumptionHandle)
        XCTAssertEqual(s.state, .idle)
    }

    func testMalformedAndErrorFrames() {
        let (s, _) = make()
        XCTAssertEqual(s.handle("not json", playback: .drained), [])
        if case .serverError? = s.handle(#"{"error":{"code":400}}"#, playback: .drained).first {
        } else {
            XCTFail("expected serverError")
        }
        XCTAssertEqual(
            s.handle(#"{"serverContent":{"modelTurn":{"parts":[{"text":"hi"}]}}}"#, playback: .drained), [],
            "non-audio parts ignored")
    }
}
