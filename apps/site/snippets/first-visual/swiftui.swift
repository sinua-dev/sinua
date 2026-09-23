import SwiftUI
import Sinua

struct BreathingOrb: View {
    var body: some View {
        SinuaOrb(pattern: .breathing)
            .frame(width: 160, height: 160)
    }
}
