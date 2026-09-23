import XCTest

@testable import SinuaVoice

/// `LiveKitAgentTracker` -- the SDK-free core of `SinuaLiveKit` -- driven by
/// fake participants and PCM, the same scenarios as the Web adapter's rules
/// (packages/voice/src/livekitAgent.ts + LiveKitVoiceSource.ts).
final class LiveKitAgentTests: XCTestCase {
    private let agentAttrs = ["lk.agent.state": "listening"]

    private func tracker() -> (LiveKitAgentTracker, Box) {
        let t = LiveKitAgentTracker()
        let box = Box()
        t.onState = { box.states.append($0) }
        t.onInterrupt = { box.interrupts += 1 }
        t.onMetrics = { box.metrics.append($0) }
        return (t, box)
    }

    final class Box {
        var states: [AgentState] = []
        var interrupts = 0
        var metrics: [VoiceMetrics] = []
    }

    private func sine(_ n: Int, amp: Float, rate: Double = 48_000, freq: Double = 440) -> [Float] {
        (0..<n).map { amp * Float(sin(2 * .pi * freq * Double($0) / rate)) }
    }

    // MARK: - rules

    func testAttributeMappingIsIdentityAndIgnoresUnknown() {
        for s in AgentState.allCases {
            XCTAssertEqual(LiveKitAgent.agentState(fromAttributes: ["lk.agent.state": s.rawValue]), s)
        }
        XCTAssertNil(LiveKitAgent.agentState(fromAttributes: ["lk.agent.state": "dancing"]))
        XCTAssertNil(LiveKitAgent.agentState(fromAttributes: [:]))
    }

    func testDiscoveryRules() {
        XCTAssertTrue(LiveKitAgent.isPrimaryAgent(isAgentKind: true, attributes: [:]))
        XCTAssertFalse(LiveKitAgent.isPrimaryAgent(isAgentKind: false, attributes: [:]))
        XCTAssertFalse(LiveKitAgent.isPrimaryAgent(isAgentKind: true, attributes: ["lk.publish_on_behalf": "a"]))
        XCTAssertTrue(
            LiveKitAgent.publishesForAgent(
                isAgentKind: true, attributes: ["lk.publish_on_behalf": "a"], agentIdentity: "a"))
        XCTAssertFalse(
            LiveKitAgent.publishesForAgent(
                isAgentKind: true, attributes: ["lk.publish_on_behalf": "b"], agentIdentity: "a"))
        XCTAssertFalse(
            LiveKitAgent.publishesForAgent(
                isAgentKind: false, attributes: ["lk.publish_on_behalf": "a"], agentIdentity: "a"))
    }

    func testBargeInTruthTable() {
        for prev in AgentState.allCases {
            for next in AgentState.allCases {
                for speaking in [false, true] {
                    let want = prev == .speaking && (next == .listening || next == .thinking) && speaking
                    XCTAssertEqual(
                        LiveKitAgent.isInferredBargeIn(prev: prev, next: next, userSpeaking: speaking), want,
                        "\(prev)->\(next) speaking=\(speaking)")
                }
            }
        }
    }

    // MARK: - tracker

    func testAdoptsPrimaryAgentNotWorkerOrStandard() {
        let (t, box) = tracker()
        t.start()
        XCTAssertEqual(t.participantSeen(identity: "user", isAgentKind: false, attributes: [:]), .other)
        XCTAssertEqual(
            t.participantSeen(identity: "avatar", isAgentKind: true, attributes: ["lk.publish_on_behalf": "agent"]),
            .other,
            "a worker seen before its agent is not adopted")
        XCTAssertNil(t.agentIdentity)
        XCTAssertEqual(t.participantSeen(identity: "agent", isAgentKind: true, attributes: agentAttrs), .agent)
        XCTAssertEqual(t.agentIdentity, "agent")
        XCTAssertEqual(
            t.role(identity: "avatar", isAgentKind: true, attributes: ["lk.publish_on_behalf": "agent"]), .agentWorker)
        XCTAssertEqual(
            t.participantSeen(identity: "agent2", isAgentKind: true, attributes: [:]), .other, "first agent wins")
        XCTAssertEqual(box.states, [.initializing, .listening])
    }

    func testAttributesLandingAfterJoinAdoptTheAgent() {
        let (t, box) = tracker()
        t.start()
        // Joined as a plain participant view (kind known, no attributes yet) -- still primary, adopted without a state.
        t.participantSeen(identity: "agent", isAgentKind: true, attributes: [:])
        XCTAssertEqual(box.states, [.initializing])
        t.attributesChanged(
            identity: "agent", isAgentKind: true, attributes: ["lk.agent.state": "thinking"],
            changed: ["lk.agent.state": "thinking"], localSpeaking: false)
        XCTAssertEqual(box.states, [.initializing, .thinking])
    }

    func testAttributesOnlyAdoptWhenNoAgentYet() {
        let (t, box) = tracker()
        t.start()
        t.attributesChanged(
            identity: "agent", isAgentKind: true, attributes: ["lk.agent.state": "speaking"],
            changed: ["lk.agent.state": "speaking"], localSpeaking: false)
        XCTAssertEqual(t.agentIdentity, "agent")
        XCTAssertEqual(box.states, [.initializing, .speaking])
    }

    func testUnrelatedAndUnknownAttributeChangesAreIgnored() {
        let (t, box) = tracker()
        t.start()
        t.participantSeen(identity: "agent", isAgentKind: true, attributes: agentAttrs)
        t.attributesChanged(
            identity: "agent", isAgentKind: true, attributes: ["lk.agent.state": "listening", "x": "1"],
            changed: ["x": "1"], localSpeaking: false)
        t.attributesChanged(
            identity: "agent", isAgentKind: true, attributes: ["lk.agent.state": "dancing"],
            changed: ["lk.agent.state": "dancing"], localSpeaking: false)
        t.attributesChanged(
            identity: "user", isAgentKind: false, attributes: ["lk.agent.state": "speaking"],
            changed: ["lk.agent.state": "speaking"], localSpeaking: false)
        XCTAssertEqual(box.states, [.initializing, .listening])
    }

    func testBargeInFiresOnlyWhenUserIsSpeaking() {
        let (t, box) = tracker()
        t.start()
        t.participantSeen(identity: "agent", isAgentKind: true, attributes: ["lk.agent.state": "speaking"])
        t.attributesChanged(
            identity: "agent", isAgentKind: true, attributes: agentAttrs,
            changed: agentAttrs, localSpeaking: false)
        XCTAssertEqual(box.interrupts, 0, "normal end of turn")
        t.attributesChanged(
            identity: "agent", isAgentKind: true, attributes: ["lk.agent.state": "speaking"],
            changed: ["lk.agent.state": "speaking"], localSpeaking: true)
        t.attributesChanged(
            identity: "agent", isAgentKind: true, attributes: agentAttrs,
            changed: agentAttrs, localSpeaking: true)
        XCTAssertEqual(box.interrupts, 1)
        XCTAssertEqual(box.states, [.initializing, .speaking, .listening, .speaking, .listening])
    }

    func testAgentLeavingReturnsToInitializingAndStopGoesIdle() {
        let (t, box) = tracker()
        t.start()
        t.participantSeen(identity: "agent", isAgentKind: true, attributes: agentAttrs)
        t.audioAttached()
        XCTAssertFalse(t.participantLeft(identity: "user"))
        XCTAssertTrue(t.participantLeft(identity: "agent"))
        XCTAssertNil(t.agentIdentity)
        XCTAssertFalse(t.hasAudio)
        t.stop()
        XCTAssertEqual(box.states, [.initializing, .listening, .initializing, .idle])
    }

    func testEnergyFallbackUntilAStateIsPublished() {
        let (t, box) = tracker()
        t.start()
        t.participantSeen(identity: "agent", isAgentKind: true, attributes: [:])
        t.audioAttached()
        let loud = sine(1600, amp: 0.5)
        loud.withUnsafeBufferPointer { t.sink.write($0) }
        t.tick()
        XCTAssertEqual(t.state, .speaking)
        [Float](repeating: 0, count: 1600).withUnsafeBufferPointer { t.sink.write($0) }
        for _ in 0..<30 { t.tick() }  // release decays the smoothed level
        XCTAssertEqual(t.state, .listening)
        // Once the agent publishes a state, energy no longer drives it.
        t.attributesChanged(
            identity: "agent", isAgentKind: true, attributes: ["lk.agent.state": "thinking"],
            changed: ["lk.agent.state": "thinking"], localSpeaking: false)
        loud.withUnsafeBufferPointer { t.sink.write($0) }
        t.tick()
        XCTAssertEqual(t.state, .thinking)
        XCTAssertEqual(box.states.first, .initializing)
    }

    func testNoMetricsWithoutAudio() {
        let (t, box) = tracker()
        t.start()
        t.participantSeen(identity: "agent", isAgentKind: true, attributes: agentAttrs)
        t.tick()
        XCTAssertTrue(box.metrics.isEmpty)
    }

    /// The sink path is the mic path: same samples in, same metrics out as feeding `SpectrumAnalyser` directly.
    func testPcmSinkMatchesDirectAnalysis() {
        let (t, box) = tracker()
        t.start()
        t.participantSeen(identity: "agent", isAgentKind: true, attributes: agentAttrs)
        t.audioAttached()
        let spectrum = SpectrumAnalyser()
        let analysis = AudioAnalysis()
        var want: [VoiceMetrics] = []
        for chunk in 0..<10 {
            let s = sine(1600, amp: Float(chunk) / 10, freq: 300 + Double(chunk) * 100)
            s.withUnsafeBufferPointer { t.sink.write($0) }
            t.tick()
            spectrum.push(s)
            want.append(analysis.read(spectrum.byteFrequencyData()))
        }
        XCTAssertEqual(box.metrics, want)
    }

    func testInt16InterleavedKeepsChannelZero() {
        let (t, box) = tracker()
        t.start()
        t.participantSeen(identity: "agent", isAgentKind: true, attributes: agentAttrs)
        t.audioAttached()
        let mono = sine(1600, amp: 0.5)
        // Channel 0 = the signal quantized to int16; channel 1 = full-scale noise that must be ignored.
        var interleaved = [Int16]()
        for v in mono {
            interleaved.append(Int16(v * 32768))
            interleaved.append(Int16.max)
        }
        interleaved.withUnsafeBufferPointer { t.sink.write(int16: $0, channels: 2) }
        t.tick()
        let spectrum = SpectrumAnalyser()
        spectrum.push(mono.map { Float(Int16($0 * 32768)) / 32768 })
        XCTAssertEqual(box.metrics, [AudioAnalysis().read(spectrum.byteFrequencyData())])
    }
}
