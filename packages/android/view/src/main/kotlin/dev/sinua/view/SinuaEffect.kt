package dev.sinua.view

import java.util.concurrent.atomic.AtomicLong

/** The one-shot feedback effects (docs/fx-view.md, *One-shot effects*). */
enum class SinuaEffect(val wire: String) {
    /** A green tint, a ring and a tick (0.9 s); spoken "Done". */
    SUCCESS("success"),

    /** A short shake with a red tint (0.5 s); spoken "Something went wrong". */
    ERROR("error"),

    /** A burst of particles (1.4 s); spoken "Well done". */
    CELEBRATE("celebrate"),
}

/**
 * An effect to play on a `SinuaView`. Each value carries a fresh id, so assigning a new one
 * plays it once, even for the same kind:
 *
 * ```kotlin
 * var effect by remember { mutableStateOf<SinuaEffectTrigger?>(null) }
 * SinuaView(pattern = "tracking", effect = effect)
 * effect = SinuaEffectTrigger(SinuaEffect.CELEBRATE)
 * ```
 */
class SinuaEffectTrigger(val kind: SinuaEffect) {
    val id: Long = next.incrementAndGet()

    override fun equals(other: Any?) = other is SinuaEffectTrigger && other.id == id

    override fun hashCode() = id.hashCode()

    private companion object {
        val next = AtomicLong()
    }
}
