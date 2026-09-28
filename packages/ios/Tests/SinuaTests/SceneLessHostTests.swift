import SwiftUI
import UIKit
import XCTest

@testable import Sinua

/// SinuaView in a scene-less UIKit host (an app-delegate app, React Native's default):
/// SwiftUI reports `scenePhase == .background` there forever, so the frame loop
/// follows `UIApplication`'s active state instead. The xctest process is such a
/// host (no scene manifest, no connected scenes). Render-only; no audio.
@MainActor
final class SceneLessHostTests: XCTestCase {
    func testRunningRule() {
        func run(_ phase: ScenePhase, scenes: Bool, active: Bool, onScreen: Bool = true, paused: Bool = false) -> Bool {
            FxActivity.running(
                onScreen: onScreen, paused: paused, scenePhase: phase, appUsesScenes: scenes, appActive: active)
        }
        // Scene-based apps: scenePhase decides, exactly as before.
        XCTAssertTrue(run(.active, scenes: true, active: true))
        XCTAssertFalse(run(.inactive, scenes: true, active: true), "an inactive scene of an active app stays paused")
        XCTAssertFalse(run(.background, scenes: true, active: true))
        // Scene-less apps: the app's active state decides.
        XCTAssertTrue(run(.background, scenes: false, active: true))
        XCTAssertFalse(run(.background, scenes: false, active: false))
        XCTAssertTrue(run(.active, scenes: false, active: false), "an active scene always runs")
        // Off screen / paused always wins.
        XCTAssertFalse(run(.active, scenes: true, active: true, onScreen: false))
        XCTAssertFalse(run(.background, scenes: false, active: true, paused: true))
    }

    func testMonitorFollowsApplicationNotifications() {
        let center = NotificationCenter()
        let m = AppActivityMonitor(center: center, usesScenes: false, initiallyActive: false)
        XCTAssertFalse(m.isActive)
        center.post(name: UIApplication.didBecomeActiveNotification, object: nil)
        XCTAssertTrue(m.isActive)
        center.post(name: UIApplication.willResignActiveNotification, object: nil)
        XCTAssertFalse(m.isActive)
        center.post(name: UIApplication.didBecomeActiveNotification, object: nil)
        center.post(name: UIApplication.didEnterBackgroundNotification, object: nil)
        XCTAssertFalse(m.isActive)
    }

    /// Frames drawn within `seconds`, or as soon as `enough` have been drawn.
    private func frames(paused: Bool, enough: Int? = nil, within seconds: TimeInterval = 1.0) -> Int {
        let box = StatsBox()
        let view = SinuaView(pattern: "composing", paused: paused, onFrame: { box.stats.append($0) }).frame(
            width: 100, height: 100)
        let window = UIWindow(frame: CGRect(x: 0, y: 0, width: 200, height: 200))
        window.rootViewController = UIHostingController(rootView: view)
        window.makeKeyAndVisible()
        let deadline = Date().addingTimeInterval(seconds)
        while Date() < deadline {
            RunLoop.main.run(until: min(deadline, Date().addingTimeInterval(0.05)))
            if let enough, box.stats.count >= enough { break }
        }
        window.isHidden = true
        window.rootViewController = nil
        return box.stats.count
    }

    func testAnimatesInASceneLessHost() {
        XCTAssertFalse(AppActivityMonitor.shared.usesScenes, "precondition: the test host has no scene manifest")
        XCTAssertTrue(AppActivityMonitor.shared.isActive, "precondition: the host app is active")
        // The bug this guards drew exactly 1 frame, however long you wait; paused draws
        // <= 2. A loaded CI runner managed 7, then 3, then 1 frame(s) in a fixed 1 s
        // window (2026-09-21, 09-24 twice) against ~60 locally, so a fixed second can't
        // tell "slow" from "stuck". Wait up to 5 s for 5 frames instead: a stuck loop
        // still ends at 1, a slow runner gets there.
        let running = frames(paused: false, enough: 5, within: 5.0)
        XCTAssertGreaterThanOrEqual(running, 5, "animates (was 1 frame before the fallback)")
        let stopped = frames(paused: true)
        print("SCENELESS frames: running \(running) (stops at 5, max 5 s), paused \(stopped) in 1 s")
        XCTAssertLessThanOrEqual(stopped, 2, "paused still stops the loop")
    }
}
