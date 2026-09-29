import SwiftUI
import Sinua

struct RulesRings: View {
    let specJSON: String   // activity-rules.fxspec.json: its `rules` pick the state from the inputs
    let steps: Double, heartRate: Double

    var body: some View {
        SinuaRing(spec: specJSON, inputs: ["steps": steps, "heartRate": heartRate])
            .frame(width: 120, height: 120)
    }
}
