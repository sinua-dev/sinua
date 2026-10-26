import SwiftUI
import Sinua

struct DailyActivity: View {
    let specJSON: String   // activity-rings.fxspec.json: its `bindings` map the inputs
    let steps: Double, activeMinutes: Double, heartRate: Double

    var body: some View {
        // Pass only your raw numbers; an input you leave out keeps its target at the design's value.
        SinuaSignal(spec: specJSON, inputs: ["steps": steps, "activeMinutes": activeMinutes, "heartRate": heartRate])
            .frame(width: 120, height: 120)
    }
}
