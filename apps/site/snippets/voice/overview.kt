package snippets.voice

import androidx.compose.foundation.layout.size
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.remember
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.generated.SinuaOrb
import dev.sinua.view.generated.SinuaOrbPattern
import dev.sinua.voice.LocalMicVoiceSource

@Composable
fun TalkingOrb(micGranted: Boolean) {   // RECORD_AUDIO, requested by your app first
    // Any VoiceSource: the mic here, or a vendor source (see the vendor pages).
    val voice = remember { LocalMicVoiceSource() }
    DisposableEffect(micGranted) {
        if (micGranted) voice.connect()   // the view binds the source; it never connects it
        onDispose { voice.disconnect() }
    }
    SinuaOrb(pattern = SinuaOrbPattern.SPEAKING, voice = voice, modifier = Modifier.size(160.dp))
}
