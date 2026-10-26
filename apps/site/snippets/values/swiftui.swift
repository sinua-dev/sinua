import SwiftUI
import Sinua

struct UploadDune: View {
    let fraction: Double   // 0 to 1

    var body: some View {
        SinuaSignal(pattern: .dune, progress: fraction)
            .frame(width: 160, height: 64)
    }
}
