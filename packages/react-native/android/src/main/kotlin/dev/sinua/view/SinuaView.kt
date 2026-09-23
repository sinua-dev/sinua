package dev.sinua.view

import android.provider.Settings
import androidx.compose.foundation.Canvas
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.runtime.Composable
import androidx.compose.runtime.LaunchedEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableLongStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.withFrameNanos
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.drawscope.inset
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.semantics
import androidx.lifecycle.Lifecycle
import androidx.lifecycle.compose.LocalLifecycleOwner
import androidx.lifecycle.compose.currentStateAsState
import dev.sinua.voice.VoiceOverrides
import dev.sinua.voice.VoiceOverridesOptions
import dev.sinua.voice.VoiceSource
import org.json.JSONObject
import uniffi.core_engine.OrbFrame
import uniffi.core_engine.VoiceStateProfile
import uniffi.core_engine.frameWithOverrides
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
 * conversation; state changes cross-fade over [crossFade]. A
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
    crossFade: Double = 0.25,
    theme: FxTheme = FxTheme.AUTO,
    paused: Boolean = false,
    reducedMotion: FxReducedMotion = FxReducedMotion.AUTO,
    contentDescription: String? = null,
    maxFps: Double? = null,
    lowPower: FxLowPower = FxLowPower.AUTO,
    onFrame: ((FxFrameStats) -> Unit)? = null,
) {
    val model = remember(spec, voice, voiceOverrides) { FxModel(FxInput.Spec(spec), voice, voiceOverrides) }
    model.crossFade = crossFade
    model.specState = state
    model.inputs = inputs
    model.voiceLevelInput = voiceLevelInput
    FxCanvas(model, modifier, theme, paused, reducedMotion, contentDescription, maxFps, lowPower, onFrame)
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
) {
    // `speed` and `overrides` are *not* part of the key: rebuilding the model would reset
    // its clock, which would jump the pose exactly when a speed change should be smooth.
    val model = remember(pattern, size, voice, voiceOverrides) {
        FxModel(FxInput.State(pattern, size, overrides, speed), voice, voiceOverrides)
    }
    model.updatePlainInput(overrides, speed)
    model.specState = state
    model.inputs = inputs
    FxCanvas(model, modifier, theme, paused, reducedMotion, contentDescription, maxFps, lowPower, onFrame)
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
    crossFade: Double = 0.25,
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
    val running = !paused && lifecycle.isAtLeast(Lifecycle.State.RESUMED)
    val reducedNow by rememberUpdatedState(reduced)
    val powerSave = rememberPowerSaveMode()
    val lowPowerOn = when (lowPower) {
        FxLowPower.ON -> true
        FxLowPower.OFF -> false
        FxLowPower.AUTO -> powerSave
    }
    val perf = remember(model, lowPowerOn, maxFps) { model.performance(lowPowerOn, maxFps) }
    model.perf = perf
    model.onFrame = onFrame
    val cap = if (reduced) min(30.0, perf.maxFps ?: 30.0) else perf.maxFps

    // Frame clock: the latest vsync time; reading it in the draw lambda
    // invalidates the Canvas each frame. Reduced motion without a voice: no loop.
    val frameNanos = remember { mutableLongStateOf(0L) }
    LaunchedEffect(running, reduced, model.hasVoice, cap) {
        model.resetTick()
        if (!running || (reduced && !model.hasVoice)) return@LaunchedEffect
        // Frame cap: skipped vsyncs don't write the frame state, so nothing redraws.
        val pacer = FramePacer(cap)
        while (true) {
            withFrameNanos {
                if (pacer.shouldDraw(it / 1e6)) {
                    frameNanos.longValue = it
                    model.pacedFrames++
                }
            }
        }
    }

    val a11y = if (label?.isEmpty() == true) {
        Modifier.clearAndSetSemantics {} // decorative
    } else {
        val text = label ?: model.defaultLabel
        Modifier.semantics {
            contentDescription = text
            role = Role.Image
        }
    }
    Canvas(modifier.then(a11y)) {
        val frame = model.frame(frameNanos.longValue, running, reduced) ?: return@Canvas
        val t1 = if (model.onFrame != null) System.nanoTime() else 0L
        // Square engine space, centered in whatever box the view has.
        val side = min(size.width, size.height)
        inset((size.width - side) / 2, (size.height - side) / 2) {
            val engineSize = model.engineSize
            if (frame.previous != null) {
                paintFxFrame(frame.previous, engineSize, dark, alphaScale = (1 - frame.blend).toFloat())
                paintFxFrame(frame.frame, engineSize, dark, alphaScale = frame.blend.toFloat())
            } else {
                paintFxFrame(frame.frame, engineSize, dark)
            }
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
    var crossFade = 0.25
    var specState: String? = null
    var inputs: Map<String, Double> = emptyMap()
    var voiceLevelInput: String? = null
    var perf = FxPerformance(null, emptyMap())

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

    private val voice: VoiceOverrides?
    val hasVoice: Boolean get() = voice != null

    internal val clock = PhaseClock()

    /**
     * The engine speed per lifecycle state of a spec, cached. [FxStatePlayer] resolves the
     * spec *with* the state and multiplies by that state's speed, so the view must use the
     * same number; the file's base speed would make a state with its own speed jump once.
     */
    private val specSpeeds = HashMap<String, Double>()
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
        voice = given ?: source?.let { VoiceOverrides.bind(it, voiceOptions(family, overrides)) }
    }

    /**
     * The effective cap + low-power overrides. FX Spec 1.2: the resolver reports
     * the cap for this power state and sheds the spec's `lowPower.disable`
     * itself ([FxStatePlayer.lowPower] passes it to every resolution).
     */
    fun performance(lowPower: Boolean, optionMaxFps: Double?): FxPerformance {
        if (player.lowPower != lowPower) specSpeeds.clear() // a state's resolved speed can change
        player.lowPower = lowPower
        val json = (input as? FxInput.Spec)?.json ?: return fxPerformance(lowPower, optionMaxFps)
        val r = resolveFxSpecWith(json, null, emptyMap(), lowPower)
        val handles = runCatching {
            JSONObject(json).optJSONObject("performance")?.has("lowPower") == true
        }.getOrDefault(false)
        return fxPerformance(lowPower, optionMaxFps, r.maxFps, handles)
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

    /** The engine speed of one lifecycle state of a spec (the product the player will use). */
    private fun specSpeed(spec: String, state: String?): Double = specSpeeds.getOrPut(state ?: "") {
        val r = resolveFxSpecWith(spec, state, emptyMap(), player.lowPower)
        if (r.ok) (resolvedOpts(r.state, r.size)?.speed ?: 1.0) * r.speed else 1.0
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

    fun frame(nanos: Long, running: Boolean, reduced: Boolean): FxFrames? {
        if (!ok) return null
        val t0 = if (onFrame != null) System.nanoTime() else 0L
        val rawDt = if (lastNanos == 0L || nanos == 0L) 0.0 else max(0.0, (nanos - lastNanos) / 1e9)
        if (nanos != 0L) lastNanos = nanos
        // Stall clamp, widened so a low cap isn't mistaken for a stall.
        if (running && !reduced) clock.advance(min(rawDt, max(MAX_DT_S, perf.maxFps?.let { 1.5 / it } ?: 0.0)))
        lastComputeStart = t0
        lastRawDt = rawDt
        // Low-power overrides sit between the spec's and the voice's.
        val voiceMap = perf.overrides + (voice?.overrides(rawDt) ?: emptyMap())
        return when (input) {
            is FxInput.Spec -> {
                val ins = HashMap(inputs)
                val v = voice
                val name = voiceLevelInput
                if (name != null && v != null) ins[name] = v.metrics.level
                player.crossFade = crossFade
                player.setState(specState ?: v?.state?.wire)
                // The player multiplies by *this state's* speed, so the view divides by the
                // same one. max(1e-9, …) only guards the division; the phase freezes at speed 0.
                val stateSpeed = specSpeed(input.json, specState ?: v?.state?.wire)
                val at = if (reduced) {
                    REDUCED_MOTION_T / max(1e-9, stateSpeed)
                } else {
                    clock.phase(stateSpeed, 1.0) / max(1e-9, stateSpeed)
                }
                player.frame(input.json, at, min(rawDt, MAX_DT_S), ins, voiceMap)
            }

            is FxInput.State -> {
                // With a lifecycle state (given, or the bound voice's), the built-in voice-state
                // profile goes *under* the app's own overrides; the voice's live keys stay last.
                val lifecycle = specState ?: voice?.state?.wire
                val profile = profile(state, lifecycle)
                val t = if (reduced) REDUCED_MOTION_T else clock.phase(presetSpeed, speed * (profile?.speed ?: 1.0))
                var merged = (profile?.overrides ?: emptyMap()) + overrides + voiceMap
                // `audioInput` names which app input drives `audioLevel` (never an engine key).
                val level = profile?.audioInput?.let { inputs[it] }
                if (level != null) merged = merged + ("audioLevel" to level)
                frameWithOverrides(state, size, t, merged)?.let { FxFrames(it, null, 1.0) }
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
 * Native counterpart of @sinua/core's `FxSpecPlayer`: the Studio's state
 * cross-fade (cubic ease-out) over `resolveFxSpecWith`, plus runtime keys
 * (the voice's) spread last.
 */
internal class FxStatePlayer {
    var crossFade = 0.25

    /** FX Spec 1.2 low power, passed to the resolver (sheds `performance.lowPower.disable`). */
    var lowPower = false
    private var current: String? = null
    private var previous: String? = null
    private var fadeAge = Double.POSITIVE_INFINITY

    fun setState(key: String?) {
        if (key == current) return
        previous = current
        current = key
        fadeAge = if (crossFade > 0) 0.0 else Double.POSITIVE_INFINITY
    }

    fun frame(
        spec: String,
        elapsed: Double,
        dt: Double,
        inputs: Map<String, Double>,
        extra: Map<String, Double>,
    ): FxFrames? {
        fadeAge += dt
        val now = render(spec, current, elapsed, inputs, extra, lowPower) ?: return null
        if (fadeAge >= crossFade) return FxFrames(now, null, 1.0)
        val prev = render(spec, previous, elapsed, inputs, extra, lowPower)
        val u = fadeAge / crossFade
        return FxFrames(now, prev, 1 - (1 - u).pow(3))
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
internal fun FxCanvasForTest(model: FxModel, maxFps: Double?) =
    FxCanvas(model, Modifier, FxTheme.LIGHT, false, FxReducedMotion.NEVER, null, maxFps, FxLowPower.OFF, null)
