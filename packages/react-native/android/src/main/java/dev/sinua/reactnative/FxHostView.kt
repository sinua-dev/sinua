package dev.sinua.reactnative

import android.content.Context
import android.os.SystemClock
import android.widget.FrameLayout
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.ComposeView
import androidx.compose.ui.platform.ViewCompositionStrategy
import dev.sinua.view.FxFrameStats
import dev.sinua.view.FxLowPower
import dev.sinua.view.FxReducedMotion
import dev.sinua.view.FxTheme
import dev.sinua.view.SinuaView
import dev.sinua.voice.LocalMicVoiceSource
import dev.sinua.voice.TestToneVoiceSource
import dev.sinua.voice.VoiceSource
import org.json.JSONObject

/**
 * The RN SinuaView's native half on Android (docs/fx-view.md, *React Native*): a
 * FrameLayout holding a ComposeView that renders packages/android's Compose
 * `SinuaView` unchanged. SinuaViewManager sets the props; they live in Compose
 * state, so a prop change recomposes in place (no remount, the clock keeps
 * running). SinuaView and dev.sinua.voice are compiled into this library by
 * build.sh (copied from packages/android), like the UniFFI bindings.
 */
class FxHostView(context: Context) : FrameLayout(context) {
    var spec by mutableStateOf<String?>(null)
    var state by mutableStateOf("working")
    var size by mutableStateOf(64)
    var overrides by mutableStateOf(emptyMap<String, Double>())
    var speed by mutableStateOf(1.0)
    var specState by mutableStateOf<String?>(null)
    var inputs by mutableStateOf(emptyMap<String, Double>())
    var voiceLevelInput by mutableStateOf<String?>(null)
    var crossFade by mutableStateOf(0.25)
    var theme by mutableStateOf(FxTheme.AUTO)
    var paused by mutableStateOf(false)
    var reducedMotion by mutableStateOf(FxReducedMotion.AUTO)
    var maxFps by mutableStateOf<Double?>(null)
    var lowPower by mutableStateOf(FxLowPower.AUTO)
    var label by mutableStateOf<String?>(null)
    var reportFrames by mutableStateOf(false)
    private var voice by mutableStateOf<VoiceSource?>(null)
    private var voiceMode = "none"
    /** A source the app created and owns (src/voice.ts): bound, never connected or disconnected here. */
    private var boundSourceId: String? = null

    /** At most 4 per second, only while [reportFrames]. */
    var onFrameStats: ((FxFrameStats) -> Unit)? = null
    private var lastFrameEvent = 0L

    init {
        addView(
            ComposeView(context).apply {
                // Fabric recycles and detaches views; dispose with the host view, not the window.
                setViewCompositionStrategy(ViewCompositionStrategy.DisposeOnDetachedFromWindowOrReleasedFromPool)
                setContent { this@FxHostView.HostContent() }
            },
            LayoutParams(LayoutParams.MATCH_PARENT, LayoutParams.MATCH_PARENT),
        )
    }

    @androidx.compose.runtime.Composable
    private fun HostContent() {
        val frame: ((FxFrameStats) -> Unit)? = if (reportFrames) ({ s -> frame(s) }) else null
        val s = spec
        if (!s.isNullOrEmpty()) {
            SinuaView(
                spec = s, voice = voice, state = specState, inputs = inputs, voiceLevelInput = voiceLevelInput,
                crossFade = crossFade, theme = theme, paused = paused, reducedMotion = reducedMotion,
                contentDescription = label, maxFps = maxFps, lowPower = lowPower, onFrame = frame,
            )
        } else {
            SinuaView(
                pattern = state, size = size.toUInt(), overrides = overrides, speed = speed, voice = voice,
                theme = theme, paused = paused, reducedMotion = reducedMotion, contentDescription = label,
                maxFps = maxFps, lowPower = lowPower, onFrame = frame,
            )
        }
    }

    private fun frame(s: FxFrameStats) {
        val now = SystemClock.uptimeMillis()
        if (now - lastFrameEvent < 250) return
        lastFrameEvent = now
        onFrameStats?.invoke(s)
    }

    /** Binds a source the app created and owns; null goes back to the `voice` shorthands. */
    fun bindVoiceSource(id: String?) {
        if (id == boundSourceId) return
        boundSourceId = id
        if (voiceMode != "none") setVoiceMode("none")
        voice = id?.let { VoiceRegistry.source(it) }
    }

    /** SinuaView draws a voice but doesn't own it: the host connects and disconnects. */
    fun setVoiceMode(mode: String) {
        if (mode == voiceMode) return
        if (boundSourceId != null && mode != "none") return // an app-owned source is bound
        voiceMode = mode
        voice?.disconnect()
        voice = null
        val source: VoiceSource = when (mode) {
            "test" -> TestToneVoiceSource()
            "mic" -> LocalMicVoiceSource()
            else -> return
        }
        try {
            source.connect()
            voice = source
        } catch (_: Exception) {
            // No mic permission / no audio: the view keeps drawing without a voice.
        }
    }

    override fun onDetachedFromWindow() {
        super.onDetachedFromWindow()
        // Only a source this view created (the `voice` shorthands) is disconnected here.
        if (boundSourceId == null) voice?.disconnect()
        voice = null
        voiceMode = "none"
        boundSourceId = null
    }

    companion object {
        fun map(json: String?): Map<String, Double> {
            if (json.isNullOrEmpty()) return emptyMap()
            return try {
                val o = JSONObject(json)
                o.keys().asSequence().mapNotNull { k -> (o.opt(k) as? Number)?.let { k to it.toDouble() } }.toMap()
            } catch (_: Exception) {
                emptyMap()
            }
        }
    }
}
