import SwiftUI
import Sinua
import SinuaVoice

struct Assistant: View {
    let voice: VoiceSource

    var body: some View {
        // It looks at you while you talk, looks away while it thinks, and its mouth
        // follows the agent's voice. `hue` turns the body; the eyes keep their colour.
        SinuaCharacter(
            pattern: .buzzy,
            hue: 190,
            voice: voice,
            accessibilityLabel: "Buzzy",
            labels: ["listening": "Buzzy is listening", "speaking": "Buzzy is speaking"],
            expression: "happy",          // a mood your app picks
            palette: ["accent": "#FFFFFF"]  // repaint a slot outright
        )
        .frame(width: 160, height: 160)
    }
}
