package snippets.platforms

import android.os.Bundle
import androidx.activity.ComponentActivity
import dev.sinua.view.SinuaViewLayout
import dev.sinua.voice.LocalMicVoiceSource
import snippets.R

class OrbActivity : ComponentActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.orb_screen)
        val orb = findViewById<SinuaViewLayout>(R.id.orb)
        // Every XML attribute is also a property; setting one redraws.
        orb.pattern = "speaking"
        orb.voice = LocalMicVoiceSource()   // connect it once RECORD_AUDIO is granted
    }
}
