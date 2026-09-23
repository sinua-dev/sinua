import SwiftUI
import Sinua

struct LowPowerOrb: View {
    var body: some View {
        // .auto (default) follows iOS Low Power Mode; .on / .off force it.
        // Low power = the spec's `performance.lowPower` block, else 30 fps with glow and particles off.
        SinuaOrb(pattern: .speaking, lowPower: .auto)
            .frame(width: 160, height: 160)
    }
}
