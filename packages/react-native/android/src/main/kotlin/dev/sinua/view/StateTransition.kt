package dev.sinua.view

import uniffi.core_engine.OrbFrame
import uniffi.core_engine.TransitionMix
import uniffi.core_engine.TransitionSide
import uniffi.core_engine.frameTransitionWithOverrides
import uniffi.core_engine.frameWithOverrides
import uniffi.core_engine.transitionMix
import uniffi.core_engine.voiceBlend
import kotlin.math.exp
import kotlin.math.min

/** The transition clock reaches ~95 % of the way when `ωt ≈ 6.3`: `ω = 6.3 / duration`. */
internal const val LAG_95 = 6.3

/** A frame's step is capped here, as the views cap the phase and the inputs. */
internal const val TRANSITION_MAX_DT = 0.1

/** A gap longer than this (back from the background): the weights land on the target. */
internal const val SETTLE_GAP_SECONDS = 1.0

/** Below this a state that is no longer the target is dropped. */
private const val GONE = 1e-4

/** The lattice-sharing orb patterns (the engine morphs them point by point). */
private val LATTICE = setOf("glowing", "calibrating", "progressing")

/**
 * State transitions, caller side -- the Kotlin mirror of `@sinua/core`'s
 * `StateTransition` (packages/core/src/transition.ts; docs/fx-spec.md, *The transition
 * contract*; design note 31). A weight per state moves on a clock of three first-order
 * lags, so a change mid-transition heads somewhere else from where it is, with no kink;
 * the engine blends a pattern's weighted sides ([voiceBlend]). Held to
 * spec/transition-timeline.json with Web and iOS.
 */
internal class StateTransition {
    private class Entry(var side: TransitionSide?, x: Double) {
        var x = x
        var l1 = x
        var l2 = x
        var v = 0.0
        var x0 = x
        var v0 = 0.0
    }

    /** The rate keys of one pattern: the last rate seen, and the sums once one changed. */
    private class Rates {
        val last = HashMap<String, Double>()
        val acc = HashMap<String, Double>()
    }

    private class Authored(val curve: String, val duration: Double, var age: Double)

    private var entries = mutableListOf<Entry>()
    private var omega = LAG_95 / 0.6

    /** An authored curve (the file wrote it): eased with velocity carried, over its duration. */
    private var authored: Authored? = null
    private var size: UInt = 64u
    private val rates = HashMap<String, Rates>()
    private var lastT: Double? = null

    /** Seconds since the last state change (cut or not); infinite before the first. */
    private var since = Double.POSITIVE_INFINITY

    /**
     * A state change happened: the weights head for the new state from where they are.
     * [duration] 0 = a cut. [authored]: the file wrote [curve] for this change, so it is
     * kept (velocity carried); otherwise the lag clock, ~95 % of the way in [duration].
     */
    fun start(duration: Double, curve: String, authored: Boolean = false) {
        if (entries.isNotEmpty()) since = 0.0
        if (!(duration > 0) || entries.isEmpty()) {
            entries = mutableListOf(Entry(null, 1.0))
            this.authored = null
            return
        }
        entries.add(Entry(null, 0.0))
        omega = LAG_95 / duration
        this.authored = if (authored) Authored(curve, duration, 0.0) else null
        for (e in entries) {
            e.x0 = e.x
            e.v0 = e.v
        }
    }

    /** Stop any transition now (reduced motion, a new design). */
    fun cancel() {
        val t = entries.lastOrNull()
        entries = if (t != null) mutableListOf(Entry(t.side, 1.0)) else mutableListOf()
        authored = null
    }

    /** No transition running: remember [to] as what is on screen. */
    fun settle(to: TransitionSide) {
        if (!active) entries = mutableListOf(Entry(to, 1.0))
    }

    fun advance(dt: Double) {
        val raw = maxOf(0.0, dt)
        since += raw
        if (raw > SETTLE_GAP_SECONDS) {
            entries.lastOrNull()?.let { entries = mutableListOf(Entry(it.side, 1.0)) }
            authored = null
            rates.clear()
            return
        }
        val step = min(raw, TRANSITION_MAX_DT)
        if (entries.size < 2 || step == 0.0) return
        val target = entries.last()
        val a = authored
        if (a != null) {
            a.age += step
            val s = min(1.0, a.age / a.duration)
            val eased = ease(a.curve, s)
            val carry = s * s * s - 2 * s * s + s
            for (e in entries) {
                val goal = if (e === target) 1.0 else 0.0
                val x = if (s >= 1) goal else e.x0 + (goal - e.x0) * eased + e.v0 * a.duration * carry
                e.v = (x - e.x) / step
                e.x = x
                e.l1 = x
                e.l2 = x
            }
            if (s >= 1) authored = null
        } else {
            val k = 1 - exp(-omega * step)
            for (e in entries) {
                val goal = if (e === target) 1.0 else 0.0
                val before = e.x
                e.l1 += (goal - e.l1) * k
                e.l2 += (e.l1 - e.l2) * k
                e.x += (e.l2 - e.x) * k
                e.v = (e.x - before) / step
            }
        }
        entries = entries.filter { it === target || it.x >= GONE || it.l1 >= GONE }.toMutableList()
        if (entries.size == 1) {
            target.x = 1.0
            target.l1 = 1.0
            target.l2 = 1.0
            target.v = 0.0
        }
    }

    /**
     * Seconds since the lifecycle state last changed, or null before the first change.
     * The frames carry it as the `stateAge` runtime key (a character blinks at the end
     * of the user's turn); other patterns ignore it.
     */
    val stateAge: Double? get() = if (since.isFinite()) since else null

    /** More than one state is on screen. */
    val active: Boolean get() = entries.size > 1

    /** The state weights, oldest first (the newest is the target); for tests and tools. */
    val weights: List<Double> get() = entries.map { it.x }

    /** The rate sums kept for [pattern] (empty until one of its rates changed). */
    fun rateSums(pattern: String): Map<String, Double> = rates[pattern]?.acc?.toMap() ?: emptyMap()

    /** The speed multiplier to run the phase at now, for [to] (the weighted speed mid-transition). */
    @Suppress("UNUSED_PARAMETER")
    fun speed(to: TransitionSide, size: UInt): Double {
        if (!active) return to.speed
        var sum = 0.0
        var total = 0.0
        for ((i, e) in entries.withIndex()) {
            val side = (if (i == entries.size - 1) to else e.side) ?: continue
            sum += e.x * side.speed
            total += e.x
        }
        return if (total > 0) sum / total else to.speed
    }

    /**
     * No transition running and the caller paints [to] itself: [to]'s overrides plus the
     * rate sums once a rate has changed (until then exactly `to.overrides`). Once a frame.
     */
    fun steadyOverrides(to: TransitionSide, size: UInt, t: Double): Map<String, Double> {
        settle(to)
        val dp = tick(t)
        val (_, overrides) = blend(to.state, listOf(entries.size - 1), size, t, dp)
        return if (rates[to.state]?.acc.isNullOrEmpty()) to.overrides else overrides
    }

    /** The frames for [to] at engine time [t]; [live] is the live runtime keys, over every side. */
    fun frames(to: TransitionSide, size: UInt, t: Double, live: Map<String, Double>): FxFrames? {
        this.size = size
        if (entries.isEmpty()) entries = mutableListOf(Entry(to, 1.0))
        val target = entries.size - 1
        entries[target].side = to
        val extra = if (since.isFinite()) live + ("stateAge" to since) else live
        val dp = tick(t)

        // One group per pattern; within it the engine blends the sides by weight.
        val groups = LinkedHashMap<String, MutableList<Int>>()
        for ((i, e) in entries.withIndex()) {
            val s = e.side ?: continue
            groups.getOrPut(s.state) { mutableListOf() }.add(i)
        }

        class Group(val pattern: String, val idx: List<Int>, val w: Double)
        val drawn =
            groups.entries
                .map { (p, idx) -> Group(p, idx, idx.sumOf { entries[it].x }) }
                .sortedByDescending { it.w } // stable: ties keep their order
                .take(2)
        val blended = drawn.map { blend(it.pattern, it.idx, size, t, dp) }

        fun draw(pattern: String, o: Map<String, Double>): OrbFrame? = frameWithOverrides(pattern, size, t, o + extra)
        if (drawn.size == 1) {
            val (m, o) = blended[0]
            val pattern = drawn[0].pattern
            if (m.structuralTo.isEmpty() || m.swap <= 0) return draw(pattern, o)?.let { FxFrames(it, null, 1.0) }
            val swapped = o + m.structuralTo
            if (m.swap >= 1) return draw(pattern, swapped)?.let { FxFrames(it, null, 1.0) }
            return draw(pattern, swapped)?.let { FxFrames(it, draw(pattern, o), m.swap) }
        }
        // Two patterns: the one holding the target fades in over the other.
        val (i, j) = if (drawn[0].idx.contains(target)) 1 to 0 else 0 to 1
        val blend = drawn[j].w / (drawn[i].w + drawn[j].w)
        if (drawn[i].pattern in LATTICE && drawn[j].pattern in LATTICE) {
            frameTransitionWithOverrides(
                TransitionSide(drawn[i].pattern, 1.0, blended[i].second + extra),
                TransitionSide(drawn[j].pattern, 1.0, blended[j].second + extra),
                size,
                t,
                blend,
            )?.let { return FxFrames(it, null, 1.0) }
        }
        return draw(drawn[j].pattern, blended[j].second)?.let {
            FxFrames(it, draw(drawn[i].pattern, blended[i].second), blend)
        }
    }

    /** Engine time moved to [t]: the step since the last frame (a jump back or a long gap restarts the sums). */
    private fun tick(t: Double): Double {
        val last = lastT
        val dp = if (last == null) 0.0 else t - last
        if (dp < 0 || dp > 1) rates.clear()
        lastT = t
        return if (dp < 0 || dp > 1) 0.0 else dp
    }

    /** A pattern's sides blended by weight, with its rate keys accumulated. */
    private fun blend(
        pattern: String,
        idx: List<Int>,
        size: UInt,
        t: Double,
        dp: Double,
    ): Pair<TransitionMix, Map<String, Double>> {
        val sides = idx.mapNotNull { entries[it].side }
        val target = entries.size - 1
        var heaviest = 0
        for (k in idx.indices) if (entries[idx[k]].x > entries[idx[heaviest]].x) heaviest = k
        val ti = idx.indexOf(target).takeIf { it >= 0 } ?: heaviest
        val m =
            voiceBlend(sides, idx.map { entries[it].x }, ti.toUInt(), size)
                ?: TransitionMix("params", 1.0, sides[0].speed, sides[0].overrides, emptyMap(), 0.0, emptyMap())
        val r = rates.getOrPut(pattern) { Rates() }
        for ((k, rate) in m.rates) {
            val a = r.acc[k]
            val before = r.last[k]
            if (a != null) {
                r.acc[k] = a + dp * rate
            } else if (before != null && before != rate) {
                // The first change of a rate: from here the view keeps the sum (until then
                // the engine's own `t × rate` is exact, so a steady view draws what it drew).
                r.acc[k] = (t - dp) * before + dp * rate
            }
            r.last[k] = rate
        }
        return m to (m.overrides + r.acc)
    }

    /** A CSS keyword curve at [s], as the engine eases it. */
    private fun ease(curve: String, s: Double): Double {
        val side = entries.lastOrNull()?.side ?: return s
        return transitionMix(side, side, size, s, curve)?.weight ?: s
    }
}
