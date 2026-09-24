package dev.sinua.minifysmoke

import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.size
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.SinuaView
import uniffi.core_engine.resolveFxSpec

/**
 * Logs `SinuaSmoke: engine ok=…` after a direct engine call, then `SinuaSmoke: frame`
 * once the view has drawn its first frame. scripts/android-minify-smoke.sh waits for
 * both; a crash, an UnsatisfiedLinkError or neither line fails it.
 */
class MainActivity : ComponentActivity() {
    private var reported = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val ok = resolveFxSpec("""{"fxSpec":"1.8","object":"orb","pattern":"glowing"}""").ok
        Log.i(TAG, "engine ok=$ok")
        setContent {
            SinuaView(
                pattern = "glowing",
                modifier = Modifier.size(200.dp),
                onFrame = {
                    if (!reported) {
                        reported = true
                        Log.i(TAG, "frame")
                    }
                },
            )
        }
    }

    private companion object {
        const val TAG = "SinuaSmoke"
    }
}
