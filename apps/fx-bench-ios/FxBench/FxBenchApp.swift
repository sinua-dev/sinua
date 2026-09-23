import Sinua
import SwiftUI

@main
struct FxBenchApp: App {
    var body: some Scene {
        WindowGroup { BenchScreen() }
    }
}

struct BenchScreen: View {
    @StateObject private var runner = BenchRunner()

    var body: some View {
        VStack(alignment: .leading, spacing: 12) {
            Text("sinua bench").font(.headline)
            Text("Runs every case twice (normal, low power). Keep the app in the foreground and don't touch the screen while it runs. Simulator numbers are not device numbers.")
                .font(.footnote).foregroundStyle(.secondary)
            HStack {
                Button("Run") { Task { await runner.run() } }.buttonStyle(.borderedProminent)
                if let path = runner.savedPath {
                    ShareLink(item: URL(fileURLWithPath: path)) { Text("Share JSON") }
                }
            }
            Text(runner.status).font(.footnote.monospaced())
            ZStack {
                if let cur = runner.current {
                    let collector = runner.collector
                    SinuaView(
                        state: cur.c.state, size: runner.file.size, overrides: cur.c.overrides,
                        reducedMotion: .never, lowPower: cur.low ? .on : .off,
                        onFrame: { collector.add($0) }
                    )
                    .id("\(cur.c.id)-\(cur.low)")
                }
            }
            .frame(width: 256, height: 256)
            List(runner.results, id: \.rowId) { r in
                HStack {
                    Text("\(r.id) · \(r.power)").font(.caption.monospaced())
                    Spacer()
                    Text(String(format: "%.0f fps · p95 %.1f ms · %d drop · cpu %.0f%%", r.fps, r.frameMs.p95, r.droppedFrames, r.cpuPct ?? 0))
                        .font(.caption.monospaced())
                }
            }
            .listStyle(.plain)
        }
        .padding()
        .task {
            if UserDefaults.standard.bool(forKey: "benchAuto") { await runner.run() }
        }
    }
}

extension CaseResult { var rowId: String { "\(id)-\(power)" } }
