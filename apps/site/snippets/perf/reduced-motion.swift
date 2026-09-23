import SwiftUI
import Sinua

struct QuietOrb: View {
    var body: some View {
        // .auto (default) follows Settings > Accessibility > Reduce Motion: a still pose.
        // With a voice attached it still redraws (up to 30 Hz): the voice cue is information.
        SinuaOrb(pattern: .working, reducedMotion: .auto)
            .frame(width: 160, height: 160)
    }
}
