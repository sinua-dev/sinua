package dev.sinua.view

import uniffi.core_engine.OrbFrame
import uniffi.core_engine.TransitionMix
import uniffi.core_engine.TransitionSide
import uniffi.core_engine.frameTransitionWithOverrides
import uniffi.core_engine.frameWithOverrides
import uniffi.core_engine.transitionMix

/** How long a loadout change takes (design note 25), seconds. */
internal const val WEAR_SECONDS = 0.35

/**
 * State transitions, caller side -- the Kotlin mirror of `@sinua/core`'s
 * `StateTransition` (packages/core/src/transition.ts; docs/fx-spec.md,
 * *Transitions*). The engine says what to draw at an instant ([transitionMix]);
 * this keeps the clock and what was on screen, so a change mid-transition starts
 * from there.
 */
internal class StateTransition {
    private var from: TransitionSide? = null

    /** What was on screen when the loadout last changed, while its change runs. */
    private var wearFrom: TransitionSide? = null
    private var wearAge = Double.POSITIVE_INFINITY
    private var shown: TransitionSide? = null
    private var age = Double.POSITIVE_INFINITY

    /** Seconds since the last state change (cut or not); infinite before the first. */
    private var since = Double.POSITIVE_INFINITY
    private var duration = 0.0
    private var curve = "easeInOut"

    /** A state change happened: animate from what is on screen now. `duration` 0 = a cut. */
    fun start(duration: Double, curve: String) {
        from = shown
        // A change of something already on screen; the first state isn't a change.
        if (shown != null) since = 0.0
        this.duration = maxOf(0.0, duration)
        this.curve = curve
        age = if (from != null && this.duration > 0) 0.0 else Double.POSITIVE_INFINITY
    }

    /**
     * The loadout changed (design note 25): the character eases from what is on screen now
     * (a hat pops in, colours blend, a new eye style swaps in a blink), on its own clock, so a
     * state change in the middle of it doesn't cut it, and the reverse.
     */
    fun wear() {
        val s = shown ?: return
        wearFrom = s
        wearAge = 0.0
    }

    /** A loadout change is easing in. */
    val wearing: Boolean get() = wearFrom != null

    /** Stop any transition now (reduced motion, a new design). */
    fun cancel() {
        age = Double.POSITIVE_INFINITY
        from = null
        wearAge = Double.POSITIVE_INFINITY
        wearFrom = null
    }

    /** No transition running: remember [to] as what is on screen. */
    fun settle(to: TransitionSide) {
        if (!active) shown = to
    }

    fun advance(dt: Double) {
        age += maxOf(0.0, dt)
        since += maxOf(0.0, dt)
        wearAge += maxOf(0.0, dt)
        if (wearAge >= WEAR_SECONDS) wearFrom = null
    }

    /**
     * Seconds since the lifecycle state last changed, or null before the first change.
     * The frames carry it as the `stateAge` runtime key (a character blinks at the end
     * of the user's turn); other patterns ignore it.
     */
    val stateAge: Double? get() = if (since.isFinite()) since else null

    val active: Boolean get() = from != null && age < duration

    private fun mix(to: TransitionSide, size: UInt): TransitionMix? {
        val f = from ?: return null
        if (!active) return null
        return transitionMix(f, to, size, age / duration, curve)
    }

    /** The speed multiplier to run the phase at now, for [to]. */
    fun speed(to: TransitionSide, size: UInt): Double = mix(to, size)?.speed ?: to.speed

    /** The frames for [to] at engine time [t]; [extra] is the live runtime keys, over both sides. */
    fun frames(to: TransitionSide, size: UInt, t: Double, live: Map<String, Double>): FxFrames? {
        val extra = if (since.isFinite()) live + ("stateAge" to since) else live
        val wf = wearFrom
        val ww = wearAge / WEAR_SECONDS

        // A loadout change in progress: the new side easing from the old one (the engine
        // answers only for two loadouts of one character; anything else draws plain).
        fun draw(s: TransitionSide): OrbFrame? {
            val o = s.overrides + extra
            if (wf != null) {
                frameTransitionWithOverrides(
                    TransitionSide(wf.state, wf.speed, wf.overrides + extra),
                    TransitionSide(s.state, s.speed, o),
                    size,
                    t,
                    ww,
                )?.let { return it }
            }
            return frameWithOverrides(s.state, size, t, o)
        }
        val f = from
        val m = mix(to, size)
        if (f == null || m == null) {
            shown = to
            return draw(to)?.let { FxFrames(it, null, 1.0) }
        }
        return when (m.technique) {
            "params" -> {
                val base = TransitionSide(to.state, m.speed, m.overrides)
                val swapped = TransitionSide(to.state, m.speed, m.overrides + m.structuralTo)
                val structural = m.structuralTo.isNotEmpty()
                shown = if (structural && m.swap >= 0.5) swapped else base
                when {
                    !structural || m.swap <= 0 -> draw(base)?.let { FxFrames(it, null, 1.0) }
                    m.swap >= 1 -> draw(swapped)?.let { FxFrames(it, null, 1.0) }
                    else -> draw(swapped)?.let { FxFrames(it, draw(base), m.swap) }
                }
            }

            "morph" -> {
                shown = to
                val withExtra = { s: TransitionSide -> TransitionSide(s.state, s.speed, s.overrides + extra) }
                frameTransitionWithOverrides(withExtra(f), withExtra(to), size, t, m.weight)?.let {
                    FxFrames(it, null, 1.0)
                }
                    ?: draw(to)?.let { FxFrames(it, draw(f), m.weight) }
            }

            else -> {
                shown = to
                draw(to)?.let { FxFrames(it, draw(f), m.weight) }
            }
        }
    }
}
