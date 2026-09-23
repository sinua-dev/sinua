package dev.sinua.view

import android.content.Context
import android.util.AttributeSet
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.platform.AbstractComposeView
import dev.sinua.voice.VoiceOverrides
import dev.sinua.voice.VoiceSource

/**
 * [SinuaView] as a classic Android `View`, for apps still on XML layouts / Views.
 * It hosts the Compose SinuaView unchanged (no new painting) through
 * `AbstractComposeView`, the documented way to put Compose inside a View
 * (developer.android.com, *Using Compose in Views*).
 *
 * ```xml
 * <dev.sinua.view.SinuaViewLayout
 *     android:id="@+id/orb"
 *     android:layout_width="160dp" android:layout_height="160dp"
 *     android:contentDescription="Assistant"
 *     app:fxPattern="speaking" app:fxTheme="auto" />
 * ```
 * Or `app:fxSpecAsset="assistant.fxspec.json"` (a file in `assets/`) with
 * `app:fxState="listening"` (the spec's lifecycle state). Every attribute has a
 * Kotlin property; setting one recomposes. A non-null [spec] takes the spec path,
 * else [pattern] is drawn.
 *
 * Needs a `ViewTreeLifecycleOwner` like any Compose-in-View: a `ComponentActivity`,
 * AppCompat 1.3+ or Fragment 1.3+ host. Give each instance a unique id (saved
 * state). The composition is disposed on detach, Compose's default strategy.
 */
class SinuaViewLayout @JvmOverloads constructor(context: Context, attrs: AttributeSet? = null, defStyleAttr: Int = 0) :
    AbstractComposeView(context, attrs, defStyleAttr) {
    /** FX Spec JSON; non-null = the spec path. */
    var spec: String? by mutableStateOf(null)

    /** Pattern name ("speaking", "tracking", ...); used when [spec] is null. */
    var pattern: String by mutableStateOf("glowing")
    var size: UInt by mutableStateOf(64u)
    var overrides: Map<String, Double> by mutableStateOf(emptyMap())
    var speed: Double by mutableStateOf(1.0)

    /** The spec's lifecycle state; null = the voice's state, else the top-level design. */
    var state: String? by mutableStateOf(null)
    var inputs: Map<String, Double> by mutableStateOf(emptyMap())
    var voiceLevelInput: String? by mutableStateOf(null)
    var crossFade: Double by mutableStateOf(0.25)
    var voice: VoiceSource? by mutableStateOf(null)
    var voiceOverrides: VoiceOverrides? by mutableStateOf(null)
    var theme: FxTheme by mutableStateOf(FxTheme.AUTO)
    var paused: Boolean by mutableStateOf(false)
    var reducedMotion: FxReducedMotion by mutableStateOf(FxReducedMotion.AUTO)
    var lowPower: FxLowPower by mutableStateOf(FxLowPower.AUTO)

    /** Frame cap; null = the display's rate. */
    var maxFps: Double? by mutableStateOf(null)
    var onFrame: ((FxFrameStats) -> Unit)? by mutableStateOf(null)

    init {
        val a = context.obtainStyledAttributes(attrs, R.styleable.SinuaViewLayout, defStyleAttr, 0)
        try {
            a.getString(R.styleable.SinuaViewLayout_fxPattern)?.let { pattern = it }
            if (a.hasValue(R.styleable.SinuaViewLayout_fxSize)) {
                size =
                    a.getInt(R.styleable.SinuaViewLayout_fxSize, 64).toUInt()
            }
            if (a.hasValue(R.styleable.SinuaViewLayout_fxSpeed)) {
                speed =
                    a.getFloat(R.styleable.SinuaViewLayout_fxSpeed, 1f).toDouble()
            }
            a.getString(R.styleable.SinuaViewLayout_fxSpec)?.let { spec = it }
            a.getString(R.styleable.SinuaViewLayout_fxSpecAsset)?.let { name ->
                spec = context.assets.open(name).bufferedReader().use { it.readText() }
            }
            a.getString(R.styleable.SinuaViewLayout_fxState)?.let { state = it }
            theme = FxTheme.entries[a.getInt(R.styleable.SinuaViewLayout_fxTheme, 0)]
            paused = a.getBoolean(R.styleable.SinuaViewLayout_fxPaused, false)
            reducedMotion = FxReducedMotion.entries[a.getInt(R.styleable.SinuaViewLayout_fxReducedMotion, 0)]
            lowPower = FxLowPower.entries[a.getInt(R.styleable.SinuaViewLayout_fxLowPower, 0)]
            val fps = a.getFloat(R.styleable.SinuaViewLayout_fxMaxFps, 0f)
            maxFps = if (fps > 0f) fps.toDouble() else null
        } finally {
            a.recycle()
        }
    }

    @Composable
    override fun Content() {
        val label = contentDescription?.toString()
        val s = spec
        if (s != null) {
            SinuaView(
                spec = s, modifier = Modifier, voice = voice, voiceOverrides = voiceOverrides, state = state,
                inputs = inputs, voiceLevelInput = voiceLevelInput, crossFade = crossFade, theme = theme,
                paused = paused, reducedMotion = reducedMotion, contentDescription = label, maxFps = maxFps,
                lowPower = lowPower, onFrame = onFrame,
            )
        } else {
            SinuaView(
                pattern = pattern, modifier = Modifier, size = size, overrides = overrides, speed = speed,
                voice = voice,
                voiceOverrides = voiceOverrides, theme = theme, paused = paused, reducedMotion = reducedMotion,
                contentDescription = label, maxFps = maxFps, lowPower = lowPower, onFrame = onFrame,
            )
        }
    }
}
