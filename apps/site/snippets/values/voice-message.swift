import SwiftUI
import Sinua

struct VoiceMessageBubble: View {
    let peaks: [Double]          // up to 64 loudness values, 0..1: your app computes them
    @Binding var progress: Double
    let seek: (Double) -> Void   // move your player

    var body: some View {
        // Drag or tap to seek; VoiceOver adjusts it like a slider.
        SinuaVoiceMessage(envelope: peaks, progress: progress) { p in seek(p) }
            .frame(width: 220, height: 40)
            .padding(10)
            .background(Color.blue.opacity(0.12), in: Capsule())
    }
}
