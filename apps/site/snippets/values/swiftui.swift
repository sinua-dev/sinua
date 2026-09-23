import SwiftUI
import Sinua

struct UploadRing: View {
    let fraction: Double   // 0 to 1

    var body: some View {
        SinuaRing(pattern: .completing, progress: .value(fraction))
            .frame(width: 64, height: 64)
    }
}

struct ActivityRings: View {
    let move: Double, exercise: Double, stand: Double

    var body: some View {
        // One value per ring, outermost first; `ringCount` sets how many are drawn.
        SinuaRing(pattern: .tracking, progress: .list([move, exercise, stand]), ringCount: 3)
            .frame(width: 96, height: 96)
    }
}
