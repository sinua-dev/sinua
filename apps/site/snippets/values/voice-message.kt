package snippets.values

import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.SinuaVoiceMessage

@Composable
fun VoiceMessageBubble(peaks: List<Double>, progress: Double, seek: (Double) -> Unit) {
    // peaks: up to 64 loudness values, 0..1, from your app. Drag or tap to seek;
    // TalkBack gets a set-progress action.
    SinuaVoiceMessage(envelope = peaks, progress = progress, onSeek = seek, modifier = Modifier.size(220.dp, 40.dp))
}
