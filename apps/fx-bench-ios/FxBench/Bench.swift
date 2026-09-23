// sinua bench, iOS (docs/bench.md): every case in spec/bench/cases.json
// through SinuaView's SinuaView twice (lowPower .off, then .on) for warmup +
// measure seconds; onFrame stats -> one result per
// spec/bench/result.schema.json, written to Documents/bench-<ts>.json.
// Launch args: `-benchAuto 1` runs on launch, `-benchSeconds N` overrides
// the measure time, `-benchCases a,b` picks cases.
import CoreEngine
import Sinua
import Foundation
import SwiftUI
import UIKit

struct BenchCase: Decodable, Identifiable {
    let id: String, label: String, state: String
    let overrides: [String: Double]
}

struct BenchFile: Decodable {
    let version: Int, size: UInt32, warmupSeconds: Double, measureSeconds: Double
    let cases: [BenchCase]
}

struct CostOut: Codable { let `class`: String; let elements: Double; let coverage: Double; let blurLoad: Double }

struct CaseResult: Codable {
    let id: String, power: String
    let targetFps: Double, frames: Int, durationS: Double, fps: Double
    let frameMs: Pct, computeMs: Pct, paintMs: Pct
    let droppedFrames: Int, hitchRatioMsPerS: Double
    let cpuPct: Double?
    let platformJankFrames: Int?
    let longAnimationFrames: Int?
    let cost: CostOut?
}

struct DeviceOut: Codable { let model: String, os: String, isSimulator: Bool, refreshHz: Double }

struct BenchResult: Codable {
    let schemaVersion: Int, platform: String, device: DeviceOut, startedAt: String
    let measureSeconds: Double, casesVersion: Int
    var cases: [CaseResult]
}

/// Frame stats land here, off SwiftUI state (no re-render per frame).
final class Collector {
    var measuring = false
    var dts: [Double] = [], computes: [Double] = [], paints: [Double] = []
    func add(_ s: FxFrameStats) {
        guard measuring else { return }
        dts.append(s.dtMs); computes.append(s.computeMs); paints.append(s.paintMs)
    }
}

@MainActor
final class BenchRunner: ObservableObject {
    @Published var current: (c: BenchCase, low: Bool)?
    @Published var status = "idle"
    @Published var results: [CaseResult] = []
    @Published var savedPath: String?
    let file: BenchFile
    let collector = Collector()
    private var running = false

    init() {
        let url = Bundle.main.url(forResource: "cases", withExtension: "json")!
        file = try! JSONDecoder().decode(BenchFile.self, from: Data(contentsOf: url))
    }

    static var isSimulator: Bool {
        #if targetEnvironment(simulator)
        true
        #else
        false
        #endif
    }

    static var model: String {
        if let sim = ProcessInfo.processInfo.environment["SIMULATOR_MODEL_IDENTIFIER"] { return sim + " (simulator)" }
        var u = utsname()
        uname(&u)
        return withUnsafeBytes(of: &u.machine) { String(decoding: $0.prefix(while: { $0 != 0 }), as: UTF8.self) }
    }

    func run() async {
        guard !running else { return }
        running = true
        defer { running = false }
        let args = UserDefaults.standard
        let measure = args.double(forKey: "benchSeconds") > 0 ? args.double(forKey: "benchSeconds") : file.measureSeconds
        let only = args.string(forKey: "benchCases")?.split(separator: ",").map(String.init)
        let cases = file.cases.filter { only == nil || only!.contains($0.id) }
        let hz = Double(UIScreen.main.maximumFramesPerSecond)
        let started = ISO8601DateFormatter().string(from: Date())
        results = []
        var k = 0
        for c in cases {
            for low in [false, true] {
                k += 1
                status = "\(k)/\(cases.count * 2) · \(c.label) · \(low ? "low" : "normal")"
                collector.measuring = false
                collector.dts = []; collector.computes = []; collector.paints = []
                current = (c, low)
                try? await Task.sleep(nanoseconds: UInt64(file.warmupSeconds * 1e9))
                let cpu0 = processCpuSeconds()
                let t0 = Date()
                collector.measuring = true
                try? await Task.sleep(nanoseconds: UInt64(measure * 1e9))
                collector.measuring = false
                let wall = Date().timeIntervalSince(t0)
                let cpu = (processCpuSeconds() - cpu0) / wall * 100
                let s = summarize(dts: collector.dts, computes: collector.computes, paints: collector.paints,
                                  durationS: wall, refreshHz: hz, cap: low ? fxDefaultLowPower.maxFps : nil)
                var eff = c.overrides
                if low { for (k2, v) in fxDefaultLowPower.overrides { eff[k2] = v } }
                let cost = estimateCost(state: c.state, size: file.size, overrides: eff).map {
                    CostOut(class: $0.class, elements: Double($0.elements), coverage: r($0.coverage), blurLoad: r($0.blurLoad))
                }
                results.append(CaseResult(
                    id: c.id, power: low ? "low" : "normal", targetFps: s.targetFps, frames: s.frames, durationS: s.durationS,
                    fps: s.fps, frameMs: s.frameMs, computeMs: s.computeMs, paintMs: s.paintMs,
                    droppedFrames: s.droppedFrames, hitchRatioMsPerS: s.hitchRatioMsPerS, cpuPct: r(cpu),
                    platformJankFrames: nil, longAnimationFrames: nil, cost: cost
                ))
            }
        }
        current = nil
        let out = BenchResult(
            schemaVersion: 1, platform: "ios",
            device: DeviceOut(model: Self.model, os: "iOS \(UIDevice.current.systemVersion)", isSimulator: Self.isSimulator, refreshHz: hz),
            startedAt: started, measureSeconds: measure, casesVersion: file.version, cases: results
        )
        let enc = JSONEncoder()
        enc.outputFormatting = [.prettyPrinted, .sortedKeys]
        let docs = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask)[0]
        let url = docs.appendingPathComponent("bench-\(started.replacingOccurrences(of: ":", with: "-")).json")
        try? enc.encode(out).write(to: url)
        savedPath = url.path
        status = "done · \(Int(hz)) Hz · \(Self.isSimulator ? "simulator: not device numbers" : "device") · saved to Files > FxBench"
        print("SINUA_BENCH_DONE \(url.path)")
    }
}
