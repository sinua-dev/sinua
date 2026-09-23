import SwiftUI
import Sinua

struct CalmOrb: View {
    var body: some View {
        // Cap the frame rate; the clock keeps wall time, so the motion's speed is unchanged.
        SinuaOrb(pattern: .breathing, maxFps: 24, onFrame: { stats in
            if stats.computeMs + stats.paintMs > 8 { print("slow frame", stats) }
        })
        .frame(width: 160, height: 160)
    }
}
