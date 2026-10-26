import SwiftUI
import Sinua

struct SignalKeys: View {
    let quality: Double, accuracy: Double, blips: Int

    var body: some View {
        // How whole the sphere is, 0 to 1: as it falls, points let go and warm to amber.
        SinuaSignal(pattern: .cohesion, quality: quality)
            .frame(width: 96, height: 96)
        // How tight the cloud is, 0 to 1: 1 is on the core.
        SinuaSignal(pattern: .halo, accuracy: accuracy)
            .frame(width: 96, height: 96)
        // Stages (2 to 8) and how far along: the clouds light up in turn.
        SinuaSignal(pattern: .constellation, progress: 0.5, steps: 4)
            .frame(width: 96, height: 96)
        // Points found (0 to 12): each flashes as the beam reaches it.
        SinuaSignal(pattern: .scan, blips: blips)
            .frame(width: 96, height: 96)
    }
}
