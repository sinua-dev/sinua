package dev.sinua.view

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.PowerManager
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.setValue
import androidx.compose.ui.platform.LocalContext
import kotlin.math.floor
import kotlin.math.max

/**
 * Frame-rate cap by skipping display frames -- the same rule as
 * @sinua/web's `createFramePacer` (docs/fx-view.md, *Performance and
 * power*): draw when `now - last >= interval - 1 ms`, then advance `last`
 * by the whole intervals the gap covers (a fixed grid: exact average rate,
 * no drift, no burst after a stall). `maxFps` null, <= 0 or non-finite:
 * every frame draws.
 */
class FramePacer(maxFps: Double?) {
    private val interval: Double? = maxFps?.takeIf { it > 0 && it.isFinite() }?.let { 1000.0 / it }
    private var last: Double? = null

    fun shouldDraw(nowMs: Double): Boolean {
        val interval = interval ?: return true
        val l = last ?: run {
            last = nowMs
            return true
        }
        val since = nowMs - l
        if (since < interval - 1) return false
        last = l + interval * max(1.0, floor((since + 1) / interval))
        return true
    }
}

/** Low-power handling: [AUTO] follows Android Battery Saver. */
enum class FxLowPower { AUTO, ON, OFF }

/** The effective cap and extra engine opts for the current power situation. */
data class FxPerformance(val maxFps: Double?, val overrides: Map<String, Double>)

/** The default when low power is on and the spec has no `performance.lowPower` (FX Spec 1.2): 30 fps, glow and particles off (docs/fx-spec.md's recommended host default). */
val FX_DEFAULT_LOW_POWER = FxPerformance(30.0, mapOf("glowStrength" to 0.0, "particleStrength" to 0.0))

/**
 * The single place FX Spec 1.2's `performance` block is honoured -- same rule
 * as @sinua/web's `performanceFor`: [specMaxFps] is the resolver's cap for
 * this power state (`FxSpecResolved.maxFps`); [specHandlesLowPower] means the
 * spec has a `performance.lowPower` block (the resolver already shed/capped);
 * otherwise low power adds [FX_DEFAULT_LOW_POWER]; the view's own `maxFps`
 * caps further (the lowest wins).
 */
fun fxPerformance(
    lowPower: Boolean,
    optionMaxFps: Double?,
    specMaxFps: Double? = null,
    specHandlesLowPower: Boolean = false,
): FxPerformance {
    val caps = mutableListOf<Double>()
    var overrides = emptyMap<String, Double>()
    specMaxFps?.takeIf { it > 0 }?.let { caps += it }
    if (lowPower && !specHandlesLowPower) {
        caps += FX_DEFAULT_LOW_POWER.maxFps!!
        overrides = FX_DEFAULT_LOW_POWER.overrides
    }
    optionMaxFps?.takeIf { it > 0 }?.let { caps += it }
    return FxPerformance(caps.minOrNull(), overrides)
}

/**
 * Battery Saver, observed: `PowerManager.isPowerSaveMode` plus the
 * `ACTION_POWER_SAVE_MODE_CHANGED` broadcast (registered while composed).
 */
@Composable
fun rememberPowerSaveMode(): Boolean {
    val context = LocalContext.current
    val pm = remember { context.getSystemService(Context.POWER_SERVICE) as PowerManager }
    var saving by remember { mutableStateOf(pm.isPowerSaveMode) }
    DisposableEffect(pm) {
        val receiver = object : BroadcastReceiver() {
            override fun onReceive(c: Context?, i: Intent?) {
                saving = pm.isPowerSaveMode
            }
        }
        context.registerReceiver(receiver, IntentFilter(PowerManager.ACTION_POWER_SAVE_MODE_CHANGED))
        saving = pm.isPowerSaveMode
        onDispose { context.unregisterReceiver(receiver) }
    }
    return saving
}

/** Per drawn frame: time since the previous drawn frame, engine time and paint time (ms). */
data class FxFrameStats(val dtMs: Double, val computeMs: Double, val paintMs: Double)
