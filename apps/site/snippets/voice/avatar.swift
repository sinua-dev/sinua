import SwiftUI
import Sinua
import SinuaVoice

struct Speaker: View {
    let voice: VoiceSource

    var body: some View {
        // The image fills the middle (innerRadius, default 0.34 of the box); the ring goes round it.
        SinuaAvatar(Image("ada"), label: "Ada", voice: voice)
            .frame(width: 56, height: 56)
    }
}
