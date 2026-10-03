package dev.sinua.view

import android.os.Build
import android.os.Handler
import android.os.Looper
import android.os.SystemClock
import android.provider.Settings
import android.view.HapticFeedbackConstants
import android.view.accessibility.AccessibilityManager
import androidx.compose.animation.core.withInfiniteAnimationFrameNanos
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.SideEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.drawscope.DrawScope
import androidx.compose.ui.graphics.drawscope.inset
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.layout.boundsInWindow
import androidx.compose.ui.layout.onGloballyPositioned
import androidx.compose.ui.layout.onSizeChanged
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.platform.LocalDensity
import androidx.compose.ui.platform.LocalView
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.dp
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.currentStateAsState
import dev.sinua.voice.SharedVoiceSource
import dev.sinua.voice.VoiceOverrides
import dev.sinua.voice.VoiceOverridesOptions
import dev.sinua.voice.VoiceSource
import org.json.JSONObject
import uniffi.core_engine.AnnouncerState
import uniffi.core_engine.FxAccessibility
import uniffi.core_engine.OrbFrame
import uniffi.core_engine.TransitionSide
import uniffi.core_engine.VoiceStateProfile
import uniffi.core_engine.a11yAccessibleName
import uniffi.core_engine.a11yAnnounceStep
import uniffi.core_engine.a11yStateWords
import uniffi.core_engine.effectInfo
import uniffi.core_engine.expressionOverrides
import uniffi.core_engine.frameWithOverrides
import uniffi.core_engine.fxSpecAccessibility
import uniffi.core_engine.fxSpecDeriveState
import uniffi.core_engine.fxSpecTransition
import uniffi.core_engine.paletteOverrides
import uniffi.core_engine.patternLayout
import uniffi.core_engine.resolveFxSpec
import uniffi.core_engine.resolveFxSpecWith
import uniffi.core_engine.resolvedOpts
import uniffi.core_engine.voiceStateProfile
import kotlin.math.max
import kotlin.math.min
import kotlin.math.pow

/** Light/dark handling. [AUTO] follows the system; `colorMode: fixed` frames look the same either way. */
enum class FxTheme { AUTO, LIGHT, DARK }

/** Reduced motion: a static frame at t = 0.6 (spec/orbs-spec.json `paint`). [AUTO] follows "Remove animations". */
enum class FxReducedMotion { AUTO, ALWAYS, NEVER }

/**
 * A drop-in view for any sinua visual: an FX Spec (JSON text) and,
 * optionally, a [VoiceSource] -- it runs the clock
 * (`t = elapsed * presetSpeed * speed`, pinned at each speed change so the pose
 * doesn't jump), themes, pauses when the lifecycle
 * drops below RESUMED, honours reduced motion, and reacts to the voice.
 *
 * ```kotlin
 * SinuaView(spec = specJson, state = "listening", voice = micSource, modifier = Modifier.size(160.dp))
 * SinuaView(pattern = "speaking")
 * ```
 *
 * [state] picks the spec's lifecycle state; with a voice it defaults to the voice's `AgentState` wire name
 * ("listening", "speaking", ...), so a v1.1 spec's `states` follow the
 * conversation. State changes animate: the same pattern interpolates its parameters, a
 * pattern change morphs (the orb lattice trio) or cross-fades, over the spec's `transitions`
 * (default 0.6 s); [crossFade] overrides every change's duration (0 = a cut). A
 * [VoiceSource] holds one callback of each kind, so the view binds it; if
 * your app already listens to the source, pass [voiceOverrides] you feed
 * yourself instead. The view never connects or disconnects the source.
 */
@Composable
fun SinuaView(
    spec: String,
    modifier: Modifier = Modifier,
    voice: VoiceSource? = null,
    voiceOverrides: VoiceOverrides? = null,
    state: String? = null,
    inputs: Map<String, Double> = emptyMap(),
    voiceLevelInput: String? = null,
    crossFade: Double? = null,
    theme: FxTheme = FxTheme.AUTO,
    paused: Boolean = false,
    reducedMotion: FxReducedMotion = FxReducedMotion.AUTO,
    contentDescription: String? = null,
    maxFps: Double? = null,
    lowPower: FxLowPower = FxLowPower.AUTO,
    onFrame: ((FxFrameStats) -> Unit)? = null,
    /** Words per state for the accessible name and announcements; win over the spec's `accessibility.states`. */
    labels: Map<String, String> = emptyMap(),
    /** Speak state changes to TalkBack (polite, rate-limited). Default: the spec's, else true. */
    announce: Boolean? = null,
    /** A light tap when the agent starts listening. Off by default; never under reduced motion. */
    haptics: Boolean = false,
    /** Derive the state from the spec's 1.9 `rules` and [inputs] (off while a voice is bound). */
    rules: Boolean = true,
    /** A one-shot effect to play (docs/fx-view.md, *One-shot effects*); each new value plays once. */
    effect: SinuaEffectTrigger? = null,
    /** Tap to hop (design note 15): a tap plays [SinuaEffect.HOP], glancing toward it. Characters only. */
    tap: Boolean = false,
    /**
     * A character's expression (design note 16): "happy", "surprised", "thoughtful", "sad",
     * "sleepy", or "none". It wins over a spec's `expression`; null lets the spec decide.
     */
    expression: String? = null,
    /**
     * A character's palette, in part (design note 19): slot -> hex, e.g. mapOf("body" to "#E63946").
     * The slots' tones follow; it wins over a spec's `palette`. A change is immediate.
     */
    palette: Map<String, String> = emptyMap(),
    /**
     * An end user's loadout (FX Spec 1.13, design note 25), with a spec that has a `wardrobe`.
     * A change eases (a hat pops in, colours blend; a cut under reduced motion). What the spec
     * no longer offers is skipped with a logged warning, and the rest applies.
     */
    loadout: SinuaLoadout? = null,
) {
    val model = remember(spec, voice, voiceOverrides) { FxModel(FxInput.Spec(spec), voice, voiceOverrides) }
    model.setLoadout(loadout)
    model.crossFade = crossFade
    model.specState = state
    model.inputs = inputs
    model.voiceLevelInput = voiceLevelInput
    model.a11yOptions(labels, announce, haptics, rules)
    SideEffect { model.play(effect) }
    model.expression = expression
    model.palette = palette
    val tapModifier = modifier.fxTapToHop(model, tap)
    FxCanvas(model, tapModifier, theme, paused, reducedMotion, contentDescription, maxFps, lowPower, onFrame)
}

/** A pattern (e.g. "speaking", "tracking") with optional engine overrides and a speed multiplier. */
@Composable
fun SinuaView(
    pattern: String,
    modifier: Modifier = Modifier,
    size: UInt = 64u,
    overrides: Map<String, Double> = emptyMap(),
    speed: Double = 1.0,
    /**
     * The agent's lifecycle state ("listening", "speaking", ...): the built-in voice-state
     * behaviour then moves this pattern, under your own [overrides]. With a [voice] attached
     * it defaults to that source's state.
     */
    state: String? = null,
    inputs: Map<String, Double> = emptyMap(),
    voice: VoiceSource? = null,
    voiceOverrides: VoiceOverrides? = null,
    theme: FxTheme = FxTheme.AUTO,
    paused: Boolean = false,
    reducedMotion: FxReducedMotion = FxReducedMotion.AUTO,
    contentDescription: String? = null,
    maxFps: Double? = null,
    lowPower: FxLowPower = FxLowPower.AUTO,
    onFrame: ((FxFrameStats) -> Unit)? = null,
    /** Words per state for the accessible name and announcements ("listening" -> "Coach is listening"). */
    labels: Map<String, String> = emptyMap(),
    /** Speak state changes to TalkBack (polite, rate-limited). Default true. */
    announce: Boolean? = null,
    /** A light tap when the agent starts listening. Off by default; never under reduced motion. */
    haptics: Boolean = false,
    /** A one-shot effect to play (docs/fx-view.md, *One-shot effects*); each new value plays once. */
    effect: SinuaEffectTrigger? = null,
    /** Tap to hop (design note 15): a tap plays [SinuaEffect.HOP], glancing toward it. Characters only. */
    tap: Boolean = false,
    /**
     * A character's expression (design note 16): "happy", "surprised", "thoughtful", "sad",
     * "sleepy", or "none". It wins over a spec's `expression`; null lets the spec decide.
     */
    expression: String? = null,
    /**
     * A character's palette, in part (design note 19): slot -> hex, e.g. mapOf("body" to "#E63946").
     * The slots' tones follow; it wins over a spec's `palette`. A change is immediate.
     */
    palette: Map<String, String> = emptyMap(),
) {
    // `speed` and `overrides` are *not* part of the key: rebuilding the model would reset
    // its clock, which would jump the pose exactly when a speed change should be smooth.
    val model = remember(pattern, size, voice, voiceOverrides) {
        FxModel(FxInput.State(pattern, size, overrides, speed), voice, voiceOverrides)
    }
    model.updatePlainInput(overrides, speed)
    model.specState = state
    model.inputs = inputs
    model.a11yOptions(labels, announce, haptics, rules = false)
    SideEffect { model.play(effect) }
    model.expression = expression
    model.palette = palette
    val tapModifier = modifier.fxTapToHop(model, tap)
    FxCanvas(model, tapModifier, theme, paused, reducedMotion, contentDescription, maxFps, lowPower, onFrame)
}

/**
 * Deprecated label: `specState` is now `state` (FX Spec 1.7 naming). [specState] sits
 * second so this overload's signature differs from the new one's (named calls don't care).
 */
@Deprecated(
    "specState is now state",
    ReplaceWith(
        "SinuaView(spec = spec, modifier = modifier, voice = voice, voiceOverrides = voiceOverrides, state = specState, inputs = inputs, voiceLevelInput = voiceLevelInput, crossFade = crossFade, theme = theme, paused = paused, reducedMotion = reducedMotion, contentDescription = contentDescription, maxFps = maxFps, lowPower = lowPower, onFrame = onFrame)",
    ),
)
@Composable
fun SinuaView(
    spec: String,
    specState: String?,
    modifier: Modifier = Modifier,
    voice: VoiceSource? = null,
    voiceOverrides: VoiceOverrides? = null,
    inputs: Map<String, Double> = emptyMap(),
    voiceLevelInput: String? = null,
    crossFade: Double? = null,
    theme: FxTheme = FxTheme.AUTO,
    paused: Boolean = false,
    reducedMotion: FxReducedMotion = FxReducedMotion.AUTO,
    contentDescription: String? = null,
    maxFps: Double? = null,
    lowPower: FxLowPower = FxLowPower.AUTO,
    onFrame: ((FxFrameStats) -> Unit)? = null,
) = SinuaView(
    spec = spec, modifier = modifier, voice = voice, voiceOverrides = voiceOverrides, state = specState,
    inputs = inputs, voiceLevelInput = voiceLevelInput, crossFade = crossFade, theme = theme,
    paused = paused, reducedMotion = reducedMotion, contentDescription = contentDescription, maxFps = maxFps,
    lowPower = lowPower, onFrame = onFrame,
)

/**
 * Deprecated label: the plain input's `state` is now `pattern` (FX Spec 1.7 naming).
 * [size] sits second so this overload's signature differs from the new one's.
 */
@Deprecated(
    "state is now pattern",
    ReplaceWith(
        "SinuaView(pattern = state, modifier = modifier, size = size, overrides = overrides, speed = speed, voice = voice, voiceOverrides = voiceOverrides, theme = theme, paused = paused, reducedMotion = reducedMotion, contentDescription = contentDescription, maxFps = maxFps, lowPower = lowPower, onFrame = onFrame)",
    ),
)
@Composable
fun SinuaView(
    state: String,
    size: UInt = 64u,
    modifier: Modifier = Modifier,
    overrides: Map<String, Double> = emptyMap(),
    speed: Double = 1.0,
    voice: VoiceSource? = null,
    voiceOverrides: VoiceOverrides? = null,
    theme: FxTheme = FxTheme.AUTO,
    paused: Boolean = false,
    reducedMotion: FxReducedMotion = FxReducedMotion.AUTO,
    contentDescription: String? = null,
    maxFps: Double? = null,
    lowPower: FxLowPower = FxLowPower.AUTO,
    onFrame: ((FxFrameStats) -> Unit)? = null,
) = SinuaView(
    pattern = state, modifier = modifier, size = size, overrides = overrides, speed = speed, voice = voice,
    voiceOverrides = voiceOverrides, theme = theme, paused = paused, reducedMotion = reducedMotion,
    contentDescription = contentDescription, maxFps = maxFps, lowPower = lowPower, onFrame = onFrame,
)

/** Tap to hop: where the tap fell in the centred square the engine draws in, -1..1 from its centre. */
private fun Modifier.fxTapToHop(model: FxModel, tap: Boolean): Modifier = if (!tap) {
    this
} else {
    pointerInput(model) {
        detectTapGestures { p ->
            val side = minOf(size.width, size.height).toFloat()
            if (side > 0f) {
                val x = (p.x - (size.width - side) / 2f) / side * 2f - 1f
                val y = (p.y - (size.height - side) / 2f) / side * 2f - 1f
                model.hop(x.coerceIn(-1f, 1f).toDouble() to y.coerceIn(-1f, 1f).toDouble())
            }
        }
    }
}

@Composable
private fun FxCanvas(
    model: FxModel,
    modifier: Modifier,
    theme: FxTheme,
    paused: Boolean,
    reducedMotion: FxReducedMotion,
    label: String?,
    maxFps: Double?,
    lowPower: FxLowPower,
    onFrame: ((FxFrameStats) -> Unit)?,
) {
    val context = LocalContext.current
    val systemReduced = remember {
        Settings.Global.getFloat(context.contentResolver, Settings.Global.ANIMATOR_DURATION_SCALE, 1f) == 0f
    }
    val reduced = when (reducedMotion) {
        FxReducedMotion.ALWAYS -> true
        FxReducedMotion.NEVER -> false
        FxReducedMotion.AUTO -> systemReduced
    }
    val dark = when (theme) {
        FxTheme.DARK -> true
        FxTheme.LIGHT -> false
        FxTheme.AUTO -> isSystemInDarkTheme()
    }
    val lifecycle by LocalLifecycleOwner.current.lifecycle.currentStateAsState()
    // On screen: a view scrolled out of a clipping parent has empty window bounds and
    // stops, like the Web and iOS views (roadmap 10). True until the first layout.
    var onScreen by remember { mutableStateOf(true) }
    val running = !paused && onScreen && lifecycle.isAtLeast(Lifecycle.State.RESUMED)
    // Short side in px, for the small-view cap (0 until measured).
    var shortPx by remember { mutableStateOf(0) }
    val smallPx = with(LocalDensity.current) { FX_SMALL_VIEW_DP.dp.toPx() }
    val small = shortPx in 1 until smallPx.toInt()
    val reducedNow by rememberUpdatedState(reduced)
    val powerSave = rememberPowerSaveMode()
    val lowPowerOn = when (lowPower) {
        FxLowPower.ON -> true
        FxLowPower.OFF -> false
        FxLowPower.AUTO -> powerSave
    }
    DisposableEffect(model) { onDispose { model.release() } }
    val perf = remember(model, lowPowerOn, maxFps, small) { model.performance(lowPowerOn, maxFps, small) }
    model.perf = perf
    model.dark = dark
    model.onFrame = onFrame
    val cap = if (reduced) min(30.0, perf.maxFps ?: 30.0) else perf.maxFps

    // Frame clock: the latest vsync time; reading it in the draw lambda
    // invalidates the Canvas each frame. Reduced motion without a voice: no loop.
    val frameNanos = remember { mutableLongStateOf(0L) }
    LaunchedEffect(running, reduced, model.hasVoice, model.effectRunning, cap) {
        model.resetTick()
        // A one-shot effect keeps the loop up under reduced motion too (its reduced variant).
        if (!running || (reduced && !model.hasVoice && !model.effectRunning)) return@LaunchedEffect
        // Frame cap: skipped vsyncs don't write the frame state, so nothing redraws.
        // An infinite animation: `withInfiniteAnimationFrameNanos` honours
        // `InfiniteAnimationPolicy`, so a host app's Compose UI tests can go idle while a
        // view animates (a plain `withFrameNanos` loop kept the Recomposer busy forever).
        val pacer = FramePacer(cap)
        while (true) {
            withInfiniteAnimationFrameNanos {
                if (pacer.shouldDraw(it / 1e6)) {
                    frameNanos.longValue = it
                    model.pacedFrames++
                }
            }
        }
    }

    // Accessibility (docs/fx-view.md): the name follows the state; changes are spoken to
    // TalkBack (only while an accessibility service is on); an opt-in tap on "listening".
    val view = LocalView.current
    val accessibility = remember { context.getSystemService(AccessibilityManager::class.java) }
    model.appLabel = label
    model.reducedNow = reduced
    model.speak = { text -> if (accessibility?.isEnabled == true) view.announceForAccessibility(text) }
    model.tap = {
        view.performHapticFeedback(
            if (Build.VERSION.SDK_INT >= 30) HapticFeedbackConstants.CONFIRM else HapticFeedbackConstants.KEYBOARD_TAP,
        )
    }
    SideEffect { model.refreshA11y() }
    val a11y = if (label?.isEmpty() == true) {
        Modifier.clearAndSetSemantics {} // decorative
    } else {
        val text = model.a11yLabel
        Modifier.semantics {
            contentDescription = text
            role = Role.Image
        }
    }
    val measure = Modifier
        .onSizeChanged { shortPx = min(it.width, it.height) }
        // Clipped away entirely (scrolled out): stop. A view with no size of its own keeps
        // the old behaviour -- there's nothing to judge by.
        .onGloballyPositioned { onScreen = it.size.width == 0 || it.size.height == 0 || !it.boundsInWindow().isEmpty }
    Canvas(modifier.then(a11y).then(measure)) {
        // A box-layout pattern (signal `playing`) gets the box ratio as `aspect`.
        val aspect = if (model.boxLayout && size.height > 0f) {
            (size.width / size.height).toDouble().coerceIn(0.125, 8.0)
        } else {
            null
        }
        val frame = model.frame(frameNanos.longValue, running, reduced, aspect) ?: return@Canvas
        val t1 = if (model.onFrame != null) System.nanoTime() else 0L
        val paint: DrawScope.() -> Unit = {
            val engineSize = model.engineSize
            if (frame.previous != null) {
                paintFxFrame(frame.previous, engineSize, dark, alphaScale = (1 - frame.blend).toFloat())
                paintFxFrame(frame.frame, engineSize, dark, alphaScale = frame.blend.toFloat())
            } else {
                paintFxFrame(frame.frame, engineSize, dark)
            }
        }
        if (model.boxLayout) {
            // The whole box at the height's scale: paintFxFrame scales by its scope's width,
            // so the scope is `height` wide; the frame itself runs `size * aspect` across.
            inset(0f, 0f, size.width - size.height, 0f, paint)
        } else {
            // Square engine space, centered in whatever box the view has.
            val side = min(size.width, size.height)
            inset((size.width - side) / 2, (size.height - side) / 2, paint)
        }
        model.onFrame?.let { cb ->
            // DrawScope records display-list ops; paintMs is the record time, not GPU raster.
            val t2 = System.nanoTime()
            cb(FxFrameStats(model.lastRawDt * 1000, (t1 - model.lastComputeStart) / 1e6, (t2 - t1) / 1e6))
        }
    }
}

internal sealed interface FxInput {
    data class Spec(val json: String) : FxInput
    data class State(val state: String, val size: UInt, val overrides: Map<String, Double>, val speed: Double) : FxInput
}

internal class FxFrames(val frame: OrbFrame, val previous: OrbFrame?, val blend: Double)

// Clock, resolved input, voice binding and the spec state cross-fade for one view.

/**
 * The view clock. `elapsed * speed` alone jumps the pose whenever the speed changes
 * (a lifecycle state with its own speed, or an app changing `speed`), because all the
 * time already elapsed is rescaled at once. So the phase is pinned at each speed change
 * and runs from there. With a constant speed the result is exactly
 * `elapsed * presetSpeed * speed`, bit for bit; `speed` 0 holds the phase.
 */
internal class PhaseClock {
    var elapsed = 0.0
        private set
    private var phaseBase = 0.0
    private var elapsedBase = 0.0
    private var lastSpeed: Double? = null

    /** `elapsed` at the previous [phase] call: a speed change counts from there. */
    private var lastRead = 0.0

    fun advance(dt: Double) {
        elapsed += dt
    }

    /** The engine time to draw at, for the preset's tuned speed and the app's multiplier. */
    fun phase(presetSpeed: Double, speed: Double): Double {
        val product = presetSpeed * speed
        if (lastSpeed != product) {
            // Pin the phase as of the previous read, then run from there at the new speed:
            // the time since that read belongs to the new speed, so `speed` 0 freezes exactly.
            val previous = lastSpeed
            if (previous != null) {
                phaseBase += (lastRead - elapsedBase) * previous
                elapsedBase = lastRead
            }
            lastSpeed = product
        }
        lastRead = elapsed
        // The engine's own factor order, so a constant speed matches `elapsed * presetSpeed * speed`.
        return phaseBase + (elapsed - elapsedBase) * presetSpeed * speed
    }
}

internal class FxModel(private val input: FxInput, source: VoiceSource?, given: VoiceOverrides?) {
    /** Overrides every state change's duration (0 = a cut); null = the spec's `transitions` / 0.6 s. */
    var crossFade: Double? = null
    var specState: String? = null
    var inputs: Map<String, Double> = emptyMap()
    var voiceLevelInput: String? = null
    var perf = FxPerformance(null, emptyMap())

    /** The view's theme is dark: a palette's dark variant is picked (design note 23). */
    var dark = false

    /** The spec draws a character (it may name a palette with a dark variant). */
    private var specIsCharacter = false

    /** The spec as drawn: the given one with the loadout applied (design note 25). */
    private var specJson: String = (input as? FxInput.Spec)?.json ?: ""
    private var loadout: SinuaLoadout? = null
    private var loadoutSet = false

    /** An end user's loadout: a change eases from what is showing. */
    fun setLoadout(l: SinuaLoadout?) {
        if (loadoutSet && l == loadout) return
        val first = !loadoutSet
        loadoutSet = true
        loadout = l
        val file = (input as? FxInput.Spec)?.json ?: return
        specJson = if (l == null) {
            file
        } else {
            val (out, warnings) = l.applyTo(file)
            if (warnings.isNotEmpty()) {
                android.util.Log.w("SinuaView", "loadout: ${warnings.joinToString { "${it.path}: ${it.message}" }}")
            }
            out
        }
        if (!first) player.wear()
    }

    /** Frames the pacer let through (tests / diagnostics). */
    internal var pacedFrames = 0
    var onFrame: ((FxFrameStats) -> Unit)? = null

    private var state = ""
    private var size = 64u
    private var speed = 1.0
    private var presetSpeed = 1.0
    private var overrides: Map<String, Double> = emptyMap()
    private var ok = false
    var defaultLabel = ""
        private set
    val engineSize: Int get() = size.toInt()

    /** The pattern fills the box ([patternLayout] == "box"): the view passes `aspect`. */
    var boxLayout = false
        private set

    // --- Accessibility (docs/fx-view.md, *Accessibility*) and the 1.9 rules glue ---
    private var labels: Map<String, String> = emptyMap()
    private var announce: Boolean? = null
    private var haptics = false
    private var rules = true
    internal var appLabel: String? = null
    internal var reducedNow = false

    /** Delivery, set by the view (and by tests): TalkBack announcement, haptic tap, the clock. */
    internal var speak: (String) -> Unit = {}
    internal var tap: () -> Unit = {}
    internal var now: () -> Double = { SystemClock.uptimeMillis() / 1000.0 }

    /** The accessible name now; Compose state, so the semantics follow it. */
    var a11yLabel by mutableStateOf("")
        private set
    private var a11yInfo = FxAccessibility(null, emptyMap(), null)
    private var a11yStarted = false
    private var a11yState: String? = null
    private var announcer = AnnouncerState(false, null, 0.0, null, 0.0)
    private var announceGen = 0
    private val handler by lazy { Handler(Looper.getMainLooper()) }
    private var offVoiceState: (() -> Unit)? = null

    /** The spec's rules: the state they picked last (hysteresis) and the inputs it was for. */
    private var derived: String? = null
    private var rulesInputs: Map<String, Double>? = null

    fun a11yOptions(labels: Map<String, String>, announce: Boolean?, haptics: Boolean, rules: Boolean) {
        val reword = labels != this.labels || announce != this.announce
        this.labels = labels
        this.announce = announce
        this.haptics = haptics
        if (rules != this.rules) rulesInputs = null
        this.rules = rules
        // Re-word the current state: a sentinel no state equals, so the next refresh runs.
        if (reword) a11yState = "\u0000"
    }

    /** With a voice bound: the app's `state` ?: the voice's. Without: the rules' state ?: the app's. */
    internal fun lifecycle(): String? {
        val v = voice
        if (v != null) return specState ?: v.state.wire
        applyRules()
        return derived ?: specState
    }

    private fun applyRules() {
        val json = (input as? FxInput.Spec)?.json
        if (json == null || !ok || !rules || voice != null) {
            derived = null
            rulesInputs = null
            return
        }
        if (inputs == rulesInputs) return
        rulesInputs = inputs
        derived = fxSpecDeriveState(json, inputs, derived)
    }

    // --- One-shot effect (docs/fx-view.md, *One-shot effects*) ---

    /** Compose state: keeps the frame loop up under reduced motion while an effect plays. */
    var effectRunning by mutableStateOf(false)
        private set
    private var effectPlayed: Long? = null
    private var effectCode = 0u
    private var effectDuration = 0.0
    private var effectStart = 0.0

    /** Plays [trigger] if it's a new one (each [SinuaEffectTrigger] value plays once). */
    fun play(trigger: SinuaEffectTrigger?) {
        if (trigger == null || trigger.id == effectPlayed) return
        effectPlayed = trigger.id
        if (trigger.kind == SinuaEffect.HOP) {
            hop(null)
            return
        }
        val info = effectInfo(trigger.kind.wire) ?: return
        tapAt = null
        effectCode = info.code
        effectDuration = info.duration
        effectStart = now()
        effectRunning = true
        // An event: spoken now, outside the state rate limit.
        val base = appLabel ?: a11yInfo.name ?: defaultLabel
        if (base.isNotEmpty() && (announce ?: a11yInfo.announce ?: true)) {
            speak(labels["effect:${trigger.kind.wire}"] ?: info.words)
        }
    }

    /** The app's palette (design note 19); empty = the character's own. */
    var palette: Map<String, String> = emptyMap()
    private var paletteFor: Pair<String, Map<String, String>>? = null
    private var paletteCache: Map<String, Double> = emptyMap()

    /** The palette's runtime keys on the drawn pattern, resolved once per (pattern, palette). */
    internal fun paletteKeys(): Map<String, Double> {
        val p = palette
        if (p.isEmpty()) return emptyMap()
        if (paletteFor != state to p) {
            paletteFor = state to p
            paletteCache = paletteOverrides(state, JSONObject(p.toSortedMap() as Map<*, *>).toString()).overrides
        }
        return paletteCache
    }

    /** The app's expression (design note 16); null = the spec's. */
    var expression: String? = null
    private var exprName: String? = null
    private var exprKnown = false
    private var exprFrom: Map<String, Double> = emptyMap()
    private var exprTo: Map<String, Double>? = null
    private var exprStart = 0.0

    /** The expression's runtime keys now, eased from what was shown over 0.6 s; none while [expression] is null. */
    internal fun expressionKeys(reduced: Boolean): Map<String, Double> {
        val now = now()
        val name = expression
        if (!exprKnown || name != exprName) {
            exprFrom = easedExpression(now, reduced) ?: emptyMap()
            exprTo = name?.let { expressionOverrides(it) ?: expressionOverrides("none") ?: emptyMap() }
            exprStart = now
            exprName = name
            exprKnown = true
        }
        return easedExpression(now, reduced) ?: emptyMap()
    }

    private fun easedExpression(now: Double, reduced: Boolean): Map<String, Double>? {
        val to = exprTo ?: return null
        val u = if (reduced) 1.0 else minOf(1.0, (now - exprStart) / 0.6)
        val e = u * u * (3 - 2 * u)
        return to.mapValues { (k, v) ->
            val from = exprFrom[k] ?: 0.0
            from + (v - from) * e
        }
    }

    /** The tap hop's tap (-1..1 from the drawn square's centre), and when the last hop began. */
    private var tapAt: Pair<Double, Double>? = null
    private var lastHop = Double.NEGATIVE_INFINITY

    /** Plays the hop (a tap at [at], or [SinuaEffect.HOP]): never over another effect, at most twice a second. */
    fun hop(at: Pair<Double, Double>?) {
        val info = effectInfo("hop") ?: return
        val now = now()
        if (effectRunning && effectCode != info.code && now - effectStart < effectDuration) return
        if (now - lastHop < 0.5) return
        lastHop = now
        tapAt = at
        effectCode = info.code
        effectDuration = info.duration
        effectStart = now
        effectRunning = true
    }

    /** The running effect's runtime keys (empty once it has ended). */
    internal fun effectKeys(reduced: Boolean): Map<String, Double> {
        if (!effectRunning) return emptyMap()
        val age = now() - effectStart
        if (age >= effectDuration) {
            effectRunning = false
            return emptyMap()
        }
        val keys = mutableMapOf(
            "effectCode" to effectCode.toDouble(),
            "effectAge" to max(0.0, age),
            "effectReduced" to if (reduced) 1.0 else 0.0,
        )
        val tap = tapAt
        if (tap != null && effectCode == effectInfo("hop")?.code) {
            keys["tapX"] = tap.first
            keys["tapY"] = tap.second
        }
        return keys
    }

    /** The state may have changed: rename the view, and let the announcer decide. */
    fun refreshA11y() {
        val st = lifecycle()
        if (a11yStarted && st == a11yState) return
        val first = !a11yStarted
        a11yStarted = true
        a11yState = st
        val base = appLabel ?: a11yInfo.name ?: defaultLabel
        a11yLabel = if (base.isEmpty()) "" else a11yAccessibleName(base, st, a11yInfo.states, labels)
        val on = base.isNotEmpty() && (announce ?: a11yInfo.announce ?: true)
        stepAnnouncer(if (on) a11yStateWords(base, st, a11yInfo.states, labels) else null)
        if (!first && st == "listening" && haptics && !reducedNow) tap()
    }

    private fun stepAnnouncer(words: String?) {
        val gen = ++announceGen
        val t = now()
        val out = a11yAnnounceStep(announcer, words, t)
        announcer = out.state
        out.announce?.let { speak(it) }
        out.recheckAt?.let { at ->
            handler.postDelayed({
                if (gen ==
                    announceGen
                ) {
                    stepAnnouncer(announcer.current)
                }
            }, ((at - t) * 1000).toLong().coerceAtLeast(0))
        }
    }

    private val voice: VoiceOverrides?
    private var tracked: SharedVoiceSource.Tracked? = null
    val hasVoice: Boolean get() = voice != null

    internal val clock = PhaseClock()

    /** The plain path's state transition (the spec path's lives in [FxStatePlayer]). */
    private val transition = StateTransition()
    private var lastLifecycle: String? = null
    private var sawLifecycle = false
    internal var lastComputeStart = 0L
    internal var lastRawDt = 0.0
    private var lastNanos = 0L
    private val player = FxStatePlayer()

    init {
        var family: String? = null
        when (input) {
            is FxInput.Spec -> {
                val json = input.json
                val r = resolveFxSpec(json)
                ok = r.ok
                if (ok) {
                    state = r.state
                    size = r.size
                    speed = r.speed
                    overrides = r.overrides
                    presetSpeed = resolvedOpts(r.state, r.size)?.speed ?: 1.0
                }
                val doc: JSONObject? = runCatching { JSONObject(json) }.getOrNull()
                family = doc?.optString("object")?.takeIf { it.isNotEmpty() }
                specIsCharacter = family == "character"
                defaultLabel = doc?.optString("name")?.takeIf { it.isNotEmpty() } ?: r.state
                if (!ok) {
                    android.util.Log.w(
                        "SinuaView",
                        "spec has errors: ${r.diagnostics.joinToString {
                            "${it.path}: ${it.message}"
                        }}",
                    )
                }
            }

            is FxInput.State -> {
                val preset = resolvedOpts(input.state, input.size)
                ok = preset != null
                state = input.state
                size = input.size
                speed = input.speed
                overrides = input.overrides
                presetSpeed = preset?.speed ?: 1.0
                defaultLabel = input.state
                if (!ok) {
                    android.util.Log.w(
                        "SinuaView",
                        "no preset for state \"${input.state}\" at size ${input.size} (unknown state, or an unsupported size: the engine resolves 20, 32 and 64)",
                    )
                }
            }
        }
        boxLayout = ok && patternLayout(state) == "box"
        // A raw source is bound through its SharedVoiceSource: this view keeps its own tracker
        // (its family's easing), and other views / a voice button keep theirs.
        tracked = if (given == null && source != null) {
            SharedVoiceSource.of(source).track(voiceOptions(family, overrides))
        } else {
            null
        }
        voice = given ?: tracked?.overrides
        a11yInfo = (input as? FxInput.Spec)?.json?.takeIf { ok }?.let { fxSpecAccessibility(it) }
            ?: FxAccessibility(null, emptyMap(), null)
        // The name and announcements follow the conversation even while the view is paused.
        offVoiceState = tracked?.source?.listenState { handler.post { refreshA11y() } }
    }

    /** Unsubscribes this view from a shared source (the model is replaced or leaves composition). */
    fun release() {
        offVoiceState?.invoke()
        offVoiceState = null
        announceGen++ // cancels a pending recheck
        tracked?.release()
        tracked = null
    }

    /**
     * The effective cap + low-power overrides. FX Spec 1.2: the resolver reports
     * the cap for this power state and sheds the spec's `lowPower.disable`
     * itself ([FxStatePlayer.lowPower] passes it to every resolution).
     */
    fun performance(lowPower: Boolean, optionMaxFps: Double?, small: Boolean = false): FxPerformance {
        player.lowPower = lowPower
        val json = (input as? FxInput.Spec)?.json
            ?: return fxPerformance(lowPower, fxOptionMaxFps(optionMaxFps, null, small))
        val r = resolveFxSpecWith(json, null, emptyMap(), lowPower)
        val handles = runCatching {
            JSONObject(json).optJSONObject("performance")?.has("lowPower") == true
        }.getOrDefault(false)
        return fxPerformance(lowPower, fxOptionMaxFps(optionMaxFps, r.maxFps, small), r.maxFps, handles)
    }

    /** Next vsync starts from a zero dt (after a pause the pose continues, doesn't jump). */
    fun resetTick() {
        lastNanos = 0L
    }

    /**
     * The built-in voice-state behaviour for plain input (`pattern` + `state`, no spec):
     * [voiceStateProfile] gives the overrides, a speed multiplier and which app input drives
     * `audioLevel`. Cached per pattern+state; null for a state outside the five voice names,
     * which leaves an app's own state names alone.
     */
    private val profiles = HashMap<String, VoiceStateProfile?>()

    private fun profile(pattern: String, state: String?): VoiceStateProfile? {
        if (state.isNullOrEmpty()) return null
        val key = "$pattern\u0000$state"
        if (profiles.containsKey(key)) return profiles[key]
        val value = voiceStateProfile(pattern, state)
        profiles[key] = value
        return value
    }

    /**
     * The plain path's design for a lifecycle state, as a transition side: the voice-state
     * profile under the app's overrides, at the effective speed. Live keys are not part of it.
     */
    private fun plainSide(lifecycle: String?): TransitionSide {
        val profile = profile(state, lifecycle)
        return TransitionSide(
            state,
            presetSpeed * speed * (profile?.speed ?: 1.0),
            (profile?.overrides ?: emptyMap()) + overrides,
        )
    }

    /**
     * Plain input only: a new `speed` / `overrides` without rebuilding the model, so the
     * clock (and the phase) survives. The pattern and size stay the model's identity.
     */
    fun updatePlainInput(overrides: Map<String, Double>, speed: Double) {
        if (input !is FxInput.State) return
        this.overrides = overrides
        this.speed = speed
    }

    fun frame(nanos: Long, running: Boolean, reduced: Boolean, aspect: Double? = null): FxFrames? {
        if (!ok) return null
        val t0 = if (onFrame != null) System.nanoTime() else 0L
        val rawDt = if (lastNanos == 0L || nanos == 0L) 0.0 else max(0.0, (nanos - lastNanos) / 1e9)
        if (nanos != 0L) lastNanos = nanos
        // Stall clamp, widened so a low cap isn't mistaken for a stall.
        if (running && !reduced) clock.advance(min(rawDt, max(MAX_DT_S, perf.maxFps?.let { 1.5 / it } ?: 0.0)))
        lastComputeStart = t0
        lastRawDt = rawDt
        // Low-power overrides sit between the spec's and the voice's.
        val live = perf.overrides + (voice?.overrides(rawDt) ?: emptyMap())
        val boxed = if (aspect != null) live + ("aspect" to aspect) else live
        // A one-shot effect the view is playing: its runtime keys.
        val keyed = boxed + effectKeys(reduced) + expressionKeys(reduced) + paletteKeys()
        // A palette's dark variant (FX Spec 1.13, design note 23): the engine picks it when told
        // `dark`. Sent only where a variant may exist, so other frames are untouched.
        val darkPalette = specIsCharacter || keyed.keys.any { it.startsWith("palette.dark.") } ||
            overrides.keys.any { it.startsWith("palette.dark.") }
        val voiceMap = if (dark && darkPalette) keyed + ("dark" to 1.0) else keyed
        return when (input) {
            is FxInput.Spec -> {
                val ins = HashMap(inputs)
                val v = voice
                val name = voiceLevelInput
                if (name != null && v != null) ins[name] = v.metrics.level
                player.crossFade = crossFade
                player.setState(lifecycle(), specJson)
                if (reduced) player.skipTransition()
                // The player multiplies by its effective speed (mixed mid-transition), so the view
                // divides by the same one. max(1e-9, …) only guards the division.
                val stateSpeed = player.speed(specJson, ins)
                val at = if (reduced) {
                    REDUCED_MOTION_T / max(1e-9, stateSpeed)
                } else {
                    clock.phase(stateSpeed, 1.0) / max(1e-9, stateSpeed)
                }
                player.frame(specJson, at, min(rawDt, MAX_DT_S), ins, voiceMap)
            }

            is FxInput.State -> {
                // With a lifecycle state (given, or the bound voice's), the built-in voice-state
                // profile goes *under* the app's own overrides; the voice's live keys stay last.
                val lifecycle = lifecycle()
                if (sawLifecycle && lifecycle != lastLifecycle) {
                    // A state change animates (0.6 s easeInOut, or `crossFade` seconds); reduced motion cuts.
                    transition.start(if (reduced) 0.0 else (crossFade ?: 0.6), "easeInOut")
                }
                sawLifecycle = true
                lastLifecycle = lifecycle
                transition.advance(min(rawDt, MAX_DT_S))
                if (reduced) transition.cancel()
                val profile = profile(state, lifecycle)
                val side = plainSide(lifecycle)
                val t = when {
                    reduced -> REDUCED_MOTION_T
                    transition.active -> clock.phase(1.0, transition.speed(side, size))
                    else -> clock.phase(presetSpeed, speed * (profile?.speed ?: 1.0))
                }
                // `audioInput` names which app input drives `audioLevel` (never an engine key).
                val level = profile?.audioInput?.let { inputs[it] }
                val withLevel = if (level != null) voiceMap + ("audioLevel" to level) else voiceMap
                // Seconds since the state changed (a character blinks at the end of the user's turn).
                val live = transition.stateAge?.let { withLevel + ("stateAge" to it) } ?: withLevel
                if (transition.active) {
                    transition.frames(side, size, t, live)
                } else {
                    transition.settle(side)
                    frameWithOverrides(state, size, t, side.overrides + live)?.let { FxFrames(it, null, 1.0) }
                }
            }
        }
    }

    companion object {
        const val MAX_DT_S = 0.1
        const val REDUCED_MOTION_T = 0.6

        /** The Studio's per-family VoiceOverrides settings (orb: raw bands; signal: scrolling history). */
        fun voiceOptions(family: String?, overrides: Map<String, Double>): VoiceOverridesOptions = when (family) {
            "orb" -> VoiceOverridesOptions(bandEaseRate = Double.POSITIVE_INFINITY)

            "signal" -> VoiceOverridesOptions(
                audioStrength = 0.0,
                historyCount = Math.round(overrides["historyCount"] ?: 40.0).toInt(),
            )

            else -> VoiceOverridesOptions()
        }
    }
}

/**
 * Native counterpart of @sinua/core's `FxSpecPlayer`: the spec's lifecycle state and its
 * transitions ([StateTransition]; the spec's 1.9 `transitions`, or [crossFade] seconds when
 * set) over `resolveFxSpecWith`, plus runtime keys (the voice's) spread last.
 */
internal class FxStatePlayer {
    /** Overrides every state change's duration (0 = a cut); null = the spec's `transitions`. */
    var crossFade: Double? = null

    /** FX Spec 1.2 low power, passed to the resolver (sheds `performance.lowPower.disable`). */
    var lowPower = false
    private var current: String? = null
    private var started = false
    private val transition = StateTransition()

    fun setState(key: String?, spec: String) {
        if (started && key == current) return
        if (started) {
            val t = fxSpecTransition(spec, current, key)
            transition.start(crossFade ?: t.duration, t.curve)
        }
        started = true
        current = key
    }

    /** End a running transition now (reduced motion). */
    fun skipTransition() = transition.cancel()

    /** The loadout changed: ease from what is showing (design note 25). */
    fun wear() = transition.wear()

    /** The current state as a transition side (effective speed), or null if the spec has errors. */
    private fun side(spec: String, inputs: Map<String, Double>): Pair<TransitionSide, UInt>? {
        val r = resolveFxSpecWith(spec, current, inputs, lowPower)
        if (!r.ok) return null
        val preset = resolvedOpts(r.state, r.size)?.speed ?: 1.0
        return TransitionSide(r.state, preset * r.speed, r.overrides) to r.size
    }

    /** The effective speed multiplier the next frame renders at (mixed mid-transition). */
    fun speed(spec: String, inputs: Map<String, Double>): Double {
        val (s, size) = side(spec, inputs) ?: return 1.0
        return transition.speed(s, size)
    }

    fun frame(
        spec: String,
        elapsed: Double,
        dt: Double,
        inputs: Map<String, Double>,
        extra: Map<String, Double>,
    ): FxFrames? {
        transition.advance(dt)
        val (s, size) = side(spec, inputs) ?: run {
            transition.cancel()
            return null
        }
        return transition.frames(s, size, elapsed * transition.speed(s, size), extra)
    }

    companion object {
        fun render(
            spec: String,
            state: String?,
            elapsed: Double,
            inputs: Map<String, Double>,
            extra: Map<String, Double>,
            lowPower: Boolean = false,
        ): OrbFrame? {
            val r = resolveFxSpecWith(spec, state, inputs, lowPower)
            if (!r.ok) return null
            val t = elapsed * (resolvedOpts(r.state, r.size)?.speed ?: 1.0) * r.speed
            return frameWithOverrides(r.state, r.size, t, r.overrides + extra)
        }
    }
}

/** Test hook: the SinuaView canvas over a given model (same code path as the public SinuaView). */
@androidx.annotation.VisibleForTesting
@Composable
internal fun FxCanvasForTest(model: FxModel, maxFps: Double?, modifier: Modifier = Modifier) =
    FxCanvas(model, modifier, FxTheme.LIGHT, false, FxReducedMotion.NEVER, null, maxFps, FxLowPower.OFF, null)
