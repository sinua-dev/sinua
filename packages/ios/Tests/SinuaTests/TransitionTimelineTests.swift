import CoreEngine
import XCTest

@testable import Sinua

/// The transition clock (design note 31) against spec/transition-timeline.json, the
/// steps Web (packages/core/test/transition-timeline.test.mjs) and Android replay too.
final class TransitionTimelineTests: XCTestCase {
    private func vectors() throws -> [[String: Any]] {
        let url = URL(fileURLWithPath: #filePath)
            .deletingLastPathComponent().deletingLastPathComponent().deletingLastPathComponent()
            .deletingLastPathComponent().deletingLastPathComponent()
            .appendingPathComponent("spec/transition-timeline.json")
        guard let data = try? Data(contentsOf: url) else {
            throw XCTSkip("spec/transition-timeline.json not readable at \(url.path) (device run?)")
        }
        let root = try JSONSerialization.jsonObject(with: data) as! [String: Any]
        return root["cases"] as! [[String: Any]]
    }

    private func side(_ o: [String: Any]) -> TransitionSide {
        TransitionSide(
            state: o["state"] as! String, speed: (o["speed"] as! NSNumber).doubleValue,
            overrides: (o["overrides"] as! [String: NSNumber]).mapValues(\.doubleValue))
    }

    func testTheClockReplaysTheSharedVectors() throws {
        for c in try vectors() {
            let name = c["name"] as! String
            let sides = (c["sides"] as! [String: [String: Any]]).mapValues(side)
            let events = c["events"] as! [[Any]]
            let dts = (c["dts"] as! [NSNumber]).map(\.doubleValue)
            let want = c["frames"] as! [[String: Any]]
            var tr = StateTransition()
            var cur = events[0][1] as! String
            var phase = (c["t0"] as! NSNumber).doubleValue * sides[cur]!.speed
            var e = 1
            for (i, dt) in dts.enumerated() {
                while e < events.count, (events[e][0] as! NSNumber).intValue == i {
                    let ev = events[e]
                    tr.start(
                        duration: (ev[2] as! NSNumber).doubleValue, curve: ev[3] as! String,
                        authored: ev.count > 4 && (ev[4] as! Bool))
                    cur = ev[1] as! String
                    e += 1
                }
                tr.advance(dt)
                let to = sides[cur]!
                phase += min(dt, 0.1) * tr.speed(to, size: 64)
                let out = tr.frames(to, size: 64, t: phase, extra: [:])
                let w = want[i]
                let ws = (w["weights"] as! [NSNumber]).map(\.doubleValue)
                let at = "\(name) frame \(i)"
                XCTAssertEqual(tr.weights.count, ws.count, at)
                for (a, b) in zip(tr.weights, ws) { XCTAssertEqual(a, b, accuracy: 1e-9, at) }
                XCTAssertEqual(tr.speed(to, size: 64), (w["speed"] as! NSNumber).doubleValue, accuracy: 1e-9, at)
                XCTAssertEqual(phase, (w["phase"] as! NSNumber).doubleValue, accuracy: 1e-9, at)
                XCTAssertEqual(out.previous != nil, w["two"] as! Bool, at)
                XCTAssertEqual(out.blend, (w["blend"] as! NSNumber).doubleValue, accuracy: 1e-9, at)
                let sums = (w["sums"] as! [String: NSNumber]).mapValues(\.doubleValue)
                let mine = tr.rateSums(to.state)
                XCTAssertEqual(Set(mine.keys), Set(sums.keys), at)
                for (k, v) in sums { XCTAssertEqual(mine[k] ?? .nan, v, accuracy: 1e-9, "\(at): \(k)") }
            }
        }
    }
}
