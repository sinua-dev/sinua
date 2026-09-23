import SwiftUI
import Sinua

struct LabelledOrb: View {
    var body: some View {
        VStack {
            // An image with this name. Default: the spec's `name`, else the pattern.
            SinuaOrb(pattern: .listening, accessibilityLabel: "Assistant is listening")
                .frame(width: 160, height: 160)
            // Decorative (the text says it already): "" hides it from VoiceOver.
            SinuaOrb(pattern: .listening, accessibilityLabel: "")
                .frame(width: 24, height: 24)
            Text("Listening…")
        }
    }
}
