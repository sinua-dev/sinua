# Device bench

A small harness that runs a fixed set of Sinua views on a real device and reports how smoothly they draw, what they cost in CPU, and whether the engine's cost badge (`estimateCost` / `fxSpecCost`) ranks them the way the device does. It exists on all three platforms with **the same cases, the same formula and the same result format**, so a phone run can be compared across platforms and against earlier runs.

| Piece | Where |
|---|---|
| Cases (8, each run at normal and low power) | [`spec/bench/cases.json`](../spec/bench/cases.json) |
| Result format (JSON Schema) | [`spec/bench/result.schema.json`](../spec/bench/result.schema.json) |
| Web | `apps/fx-bench-web` (Vite; `@sinua/web`'s `mount`) |
| iOS | `apps/fx-bench-ios` (xcodegen; SwiftUI `FxView` from the `CoreEngine` package) |
| Android | `apps/fx-bench-android` (a subproject of `packages/android`; Compose `FxView`) |
| Report / compare | [`scripts/bench/report.mjs`](../scripts/bench/report.mjs) (node, no dependencies) |

**Simulator, emulator and desktop-browser numbers are not device numbers.** The apps record `device.isSimulator`, and the report prints the label on every such table. Treat them as a check that the harness works, and at most as relative hints.

## What it measures

Every case runs through the platform's own `FxView` (the drop-in view apps ship, [`fx-view.md`](fx-view.md)), twice:
- **normal:** `lowPower` off;
- **low:** `lowPower` on. Without a spec `performance` block this means a 30 fps cap with glow and particles off (the documented host default).

Each run is a 1 s warmup, then the measure window: 10 s by default, overridable. `FxView`'s `onFrame` gives, per drawn frame, `dtMs` (the frame interval), `computeMs` (the engine) and `paintMs` (recording the drawing; not GPU raster).

**The formula**, identical in `metrics.ts`, `Metrics.swift` and `Metrics.kt`:
- `targetFps = min(refreshHz, cap)`, where cap is 30 in low power, and `interval = 1000 / targetFps`;
- a frame's lateness is `late = max(0, round(dt / interval) − 1)`, the whole display intervals it missed;
- `droppedFrames = Σ late`, `hitchRatioMsPerS = Σ late · interval / seconds`;
- percentiles are nearest-rank p50 / p95 / p99 / max, of dt, compute and paint.

The hitch ratio is Apple's **hitch time ratio** (ms of hitch per second, WWDC20 *Eliminate animation hitches with XCTest*). Apple's bands, which the report flags:

| ms/s | band |
|---|---|
| ≤ 5 | good |
| 5–10 | warning |
| > 10 | critical |

It is vsync-quantized on purpose. Summing raw `dt − interval` counted ordinary rAF timestamp jitter (17.3 vs 16.7 ms) as 3–10 ms/s of "hitch" with no frame actually late; that was the first desktop run, fixed before landing.

**Per platform extras:**

| | CPU % (process CPU time / wall × 100; can exceed 100 on several cores) | platform jank | refresh rate |
|---|---|---|---|
| iOS | `getrusage(RUSAGE_SELF)` user + sys | — | `UIScreen.maximumFramesPerSecond` (ProMotion enabled via `CADisableMinimumFrameDurationOnPhone`) |
| Android | `Process.getElapsedCpuTime()` | JankStats `isJank` count (FrameMetrics; a frame taking 2× the refresh interval) | `Display.refreshRate` |
| Web | not exposed (null) | Long Animation Frames count (≥ 50 ms, where supported) | median rAF interval over 0.5 s |

- **GPU and energy are not measured in-app.** No platform exposes them to an app in a way that is both per-app and portable. Use the platform tools below.
- **Cost:** each result carries the engine's `estimateCost` for the effective overrides. In low power, glow and particles are 0 there too. The report correlates it with the measured work.

## How to run it on your phone

Plug the phone in, close other apps, set brightness to a fixed level, and don't touch the screen while it runs. A full run is 8 cases × 2 × 11 s ≈ 3 minutes.

**iPhone**
1. `cd apps/fx-bench-ios && xcodegen generate`, then open `FxBench.xcodeproj`.
2. Select your iPhone, set your signing team on the FxBench target, and choose the **Release** configuration (Product → Scheme → Edit Scheme → Run → Build Configuration). Debug builds measure the debugger too.
3. Run, then tap **Run** in the app. When it says *done*, tap **Share JSON**. The file also appears in the Files app under *On My iPhone → FxBench*.
4. Optional: run it again with Low Power Mode on in Settings. The *low* rows force the view's low power either way; the system mode also changes CPU clocks.

**Android**
1. `cd packages/android && ./gradlew :fx-bench-android:installRelease`. Release is signed with your local debug key, so no signing setup is needed. Use release: debug builds run Compose unoptimized.
2. Open **FxBench** and tap **Run**. When it says *done*, pull the file:
   `adb pull /sdcard/Android/data/dev.sinua.fxbench/files/ bench-android/`. Or run it headless:
   `adb shell am start -n dev.sinua.fxbench/.BenchActivity --ez benchAuto true`, then wait for `SINUA_BENCH_DONE` in `adb logcat -s FxBench`.

**Web (phone browser)**
1. `cd apps/fx-bench-web && npm install && npm run build && npx vite preview --host`.
2. Open the printed network URL on the phone (the same Wi-Fi), tap **Run**, then **Download JSON**.
3. `?seconds=5` shortens each case; `?cases=orbs-breathing,ring-holo-hues` picks cases; `?auto=1` starts at once.

**Read the result**
```bash
node scripts/bench/report.mjs bench-ios.json bench-android.json   # tables, hitch bands, cost check
node scripts/bench/report.mjs --compare before.json after.json    # per-case fps / p95 / work, B vs A
node scripts/bench/report.mjs --check result.json                 # schema only
```
Under each table, the **cost check** gives the Spearman rank correlation between the badge (elements) and the measured work (compute p95 + paint p95) at normal power, where 1 means the badge orders the cases exactly as the device does. It also lists **inversions**: a case the badge calls lighter that measures more than 1.5× heavier.

## Deeper dives (the platform tools)

These are the standard tools, if a case looks bad on the phone:
- **Android:** Jetpack Macrobenchmark `FrameTimingMetric` (`frameDurationCpuMs`, `frameOverrunMs`, API 31+; "focus on P95 and P99") and `PowerMetric` (physical Pixel 6 and newer only, system-wide). Both run from a separate benchmark module on a host.
- **iOS:** Instruments' Animation Hitches and Energy Log templates; `XCTOSSignpostMetric` in an XCTest performance test (hitch count, hitch time ratio, frame rate). Measure on the device, in Release.
- **Web:** the Performance panel, and `PerformanceObserver` with `long-animation-frame`.

We didn't build the harness on Macrobenchmark or XCTest perf tests because both need a host and a separate test target, while this harness is meant to be a tap on the phone.

## First runs (2026-09-19): harness checks, not device numbers

The three runs used 2–3 s per case, on an M-series Mac: Chrome for Testing headless, the iPhone 17 simulator (iOS 26.4) and the Nutp_Test emulator (API 35, arm64, settled 90 s after boot).
- **The harness works end to end on all three:**
  - every result passes `report.mjs --check`;
  - low power holds ~30 fps on each platform;
  - the heavy glow case has the highest CPU on iOS (35%) and Android (105%).
- **Cost check:** Spearman ρ between the badge and the measured work was 0.90 on the iOS simulator, 0.78 on the emulator and 0.54 in desktop Chrome. There were no inversions in the settled runs.
- **Worth checking on a device:**
  - On the emulator, `ring-holo-hues` stayed at ~20 fps at both power levels while its compute + paint recording was ~2 ms. The time goes to raster, which suggests the per-vertex layer (Compose `saveLayer`) is expensive on the emulator's GPU. The iOS simulator drew the same case at 60 fps. Reported to voice-adapters, who own the painters.
  - Liquid is the most compute-heavy case (2–3 ms, and up to 54 ms on a freshly booted emulator), but its badge is `light`, with 29 elements. The badge counts draw calls, not engine work.
  - Headless Chrome's 30 fps low-power runs occasionally showed a 50 ms frame (3–8 per 2 s). It didn't happen in an earlier run, so it looks like headless timer noise. Check it in a real browser.
