import XCTest

@testable import SinuaVoice

/// Parity against `spec/voice-golden.json` (packages/core/scripts/gen-voice-golden.mjs):
/// Chrome's own AnalyserNode bytes + the Web Studio's analysis.ts metrics for a
/// fixed signal, and Web VoiceOverrides' maps for a scripted feed. Read via
/// `#filePath` like SinuaGoldenTests (simulator runs see the repo).
final class VoiceGoldenTests: XCTestCase {
    private func golden() throws -> [String: Any] {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("spec/voice-golden.json")
        guard let data = try? Data(contentsOf: url) else {
            throw XCTSkip("spec/voice-golden.json not readable at \(url.path) (device run?)")
        }
        return try JSONSerialization.jsonObject(with: data) as! [String: Any]
    }

    private func num(_ v: Any?) -> Double? {
        if let s = v as? String, s == "inf" { return .infinity }
        return (v as? NSNumber)?.doubleValue
    }

    func testSpectrumAndAnalysisMatchChromeAndWeb() throws {
        let a = try golden()["analysis"] as! [String: Any]
        let raw = Data(base64Encoded: a["samplesF32Base64"] as! String)!
        let samples: [Float] = raw.withUnsafeBytes { Array($0.bindMemory(to: Float.self)) }
        let frames = a["frames"] as! [[String: Any]]
        let spectrum = SpectrumAnalyser(fftSize: a["fftSize"] as! Int)
        let analysis = AudioAnalysis()
        var pos = 0
        var offByOne = 0
        var total = 0
        var maxMetricErr = 0.0
        for f in frames {
            let end = f["end"] as! Int
            spectrum.push(samples[pos..<end])
            pos = end
            let bytes = spectrum.byteFrequencyData()
            let want = f["bytes"] as! [Int]
            XCTAssertEqual(bytes.count, want.count)
            for k in 0..<want.count {
                let d = abs(Int(bytes[k]) - want[k])
                XCTAssertLessThanOrEqual(d, 1, "end \(end) bin \(k)")
                if d > 0 { offByOne += 1 }
                total += 1
            }
            let m = analysis.read(bytes)
            let level = f["level"] as! Double
            maxMetricErr = max(maxMetricErr, abs(m.level - level))
            for (b, v) in (f["bands"] as! [Double]).enumerated() {
                maxMetricErr = max(maxMetricErr, abs(m.bands[b] - v))
            }
            XCTAssertEqual(m.level > speakingLevel, level > speakingLevel, "state heuristic at end \(end)")
        }
        // float32 vs Chrome's float32: floor() boundaries only, rare.
        XCTAssertLessThan(Double(offByOne) / Double(total), 0.005, "\(offByOne)/\(total) bins off by one")
        XCTAssertLessThan(maxMetricErr, 0.01)
        print("SinuaVoice parity: \(offByOne)/\(total) bins off by one, max metric err \(maxMetricErr)")
    }

    func testAnalysisOnChromeBytesIsExact() throws {
        let frames = (try golden()["analysis"] as! [String: Any])["frames"] as! [[String: Any]]
        let analysis = AudioAnalysis()
        for f in frames {
            let m = analysis.read((f["bytes"] as! [Int]).map { UInt8($0) })
            XCTAssertEqual(m.level, f["level"] as! Double, accuracy: 1e-12)
            for (b, v) in (f["bands"] as! [Double]).enumerated() { XCTAssertEqual(m.bands[b], v, accuracy: 1e-12) }
        }
    }

    func testVoiceOverridesMatchWeb() throws {
        let o = try golden()["overrides"] as! [String: Any]
        let steps = o["steps"] as! [[String: Any]]
        let configs = o["configs"] as! [String: [String: Any]]
        let expected = o["expected"] as! [String: [[String: Double]]]
        for (name, c) in configs {
            var opts = VoiceOverridesOptions()
            if let v = num(c["audioStrength"]) { opts.audioStrength = v }
            if let v = num(c["levelEaseRate"]) { opts.levelEaseRate = v }
            if let v = num(c["bandEaseRate"]) { opts.bandEaseRate = v }
            if let v = num(c["historyCount"]) { opts.historyCount = Int(v) }
            if let v = num(c["historyHz"]) { opts.historyHz = v }
            if let v = num(c["mutedTint"]) { opts.mutedTint = v }
            if let v = num(c["mutedHue"]) { opts.mutedHue = v }
            if let i = c["interrupt"] {
                if let d = i as? [String: Any] {
                    opts.interrupt = InterruptOptions(
                        window: num(d["window"]) ?? 1, duration: num(d["duration"]), strength: num(d["strength"]),
                        tint: num(d["tint"]), hue: num(d["hue"]))
                } else {
                    opts.interrupt = nil
                }
            }
            let v = VoiceOverrides(options: opts)
            for (i, st) in steps.enumerated() {
                if let m = st["metrics"] as? [String: Any] {
                    v.push(VoiceMetrics(level: num(m["level"])!, bands: (m["bands"] as! [NSNumber]).map(\.doubleValue)))
                }
                if let s = st["state"] as? String { v.setState(AgentState(rawValue: s)!) }
                if st["interrupt"] != nil { v.interrupt() }
                if let h = num(st["historyCount"]) { v.setHistoryCount(Int(h)) }
                if let m = st["muted"] as? Bool { v.muted = m }
                if st["reset"] != nil { v.reset() }
                let got = v.overrides(dt: num(st["dt"])!)
                let want = expected[name]![i]
                XCTAssertEqual(Set(got.keys), Set(want.keys), "\(name) step \(i)")
                for (k, w) in want { XCTAssertEqual(got[k] ?? .nan, w, accuracy: 1e-9, "\(name) step \(i) \(k)") }
            }
        }
    }

    func testToneBurstsRiseAndDip() {
        // random 0.5 -> burst 0.55 s, gap 0.4 s. With Web's release (0.12 per
        // 30 Hz tick) the level needs ~0.63 s to fall under the 0.08
        // speaking threshold, so -- exactly as on Web -- a 0.4 s gap dips but
        // doesn't reach `listening`; assert the dip, not a state change.
        var g = TestToneGenerator(sampleRate: 48_000, random: { 0.5 })
        let spectrum = SpectrumAnalyser()
        let analysis = AudioAnalysis()
        var levels: [Double] = []
        for _ in 0..<60 {  // 2 s at 30 Hz
            spectrum.push(g.next(1600))
            levels.append(analysis.read(spectrum.byteFrequencyData()).level)
        }
        let peak = levels[0..<15].max()!
        let gapMin = levels[17..<29].min()!
        XCTAssertGreaterThan(peak, speakingLevel, "the first burst speaks")
        XCTAssertLessThan(gapMin, peak * 0.6, "the gap dips (peak \(peak), gap min \(gapMin))")
        XCTAssertGreaterThan(levels[29..<45].max()!, gapMin * 1.5, "the next burst rises again")
    }

    func testSampleRingDrainsOldestFirstAndKeepsNewest() {
        let ring = SampleRing(capacity: 4)
        [Float(1), 2, 3, 4, 5, 6].withUnsafeBufferPointer { ring.write($0) }
        XCTAssertEqual(ring.drain(), [3, 4, 5, 6])
        XCTAssertEqual(ring.drain(), [])
    }
}
