import SwiftUI
import Sinua

struct DailyRings: View {
    let specJSON: String   // activity-rings.fxspec.json: its `bindings` map the inputs
    let steps: Double, waterMl: Double, activeMinutes: Double

    var body: some View {
        // Pass only your raw numbers; an input you leave out keeps its target at the design's value.
        SinuaRing(spec: specJSON, inputs: ["steps": steps, "waterMl": waterMl, "activeMinutes": activeMinutes])
            .frame(width: 120, height: 120)
    }
}
