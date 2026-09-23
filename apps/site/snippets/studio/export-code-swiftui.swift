import SwiftUI
import Sinua

struct Visual: View {
    var body: some View {
        SinuaView(pattern: "working", overrides: ["glowRadius": 3, "glowStrength": 0.6]).frame(width: 160, height: 160)
    }
}

// Or use the file: Spec menu → orb-working.fxspec.json (File tab).
