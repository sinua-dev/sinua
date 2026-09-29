package snippets.perf

import androidx.compose.runtime.Composable
import dev.sinua.view.SinuaView

@Composable
fun CoachView() {
    SinuaView(
        pattern = "glowing",
        contentDescription = "Coach",
        labels = mapOf("listening" to "Koç dinliyor", "speaking" to "Koç konuşuyor"), // your words, per state
        announce = true, // state changes are spoken (TalkBack), politely
        haptics = true, // a light tap when the agent starts listening
    )
}
