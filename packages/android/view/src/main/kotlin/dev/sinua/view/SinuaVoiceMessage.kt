package dev.sinua.view

import androidx.compose.foundation.gestures.detectHorizontalDragGestures
import androidx.compose.foundation.gestures.detectTapGestures
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.runtime.Composable
import androidx.compose.runtime.getValue
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.ui.Modifier
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.ProgressBarRangeInfo
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.progressBarRangeInfo
import androidx.compose.ui.semantics.setProgress
import androidx.compose.ui.unit.IntSize
import uniffi.core_engine.playbackSeekProgress

/** At most this many envelope values (`envelope0..envelope63`). */
const val SINUA_VOICE_MESSAGE_MAX_ENVELOPE = 64

/** The position (0..1) under a point [x] px from the left of a [size] box, on the engine's row of bars. */
fun sinuaVoiceMessageSeek(size: IntSize, x: Float): Double {
    if (size.width <= 0 || size.height <= 0) return 0.0
    return playbackSeekProgress(size.width.toDouble() / size.height, x.toDouble() / size.height)
}

/** The engine keys for an envelope and a position: `progress` (clamped) and `envelope0..`. */
fun sinuaVoiceMessageKeys(
    envelope: List<Double>,
    progress: Double,
    overrides: Map<String, Double> = emptyMap(),
): Map<String, Double> {
    val o = HashMap(overrides)
    o["progress"] = progress.coerceIn(0.0, 1.0)
    envelope.take(SINUA_VOICE_MESSAGE_MAX_ENVELOPE).forEachIndexed { i, v -> o["envelope$i"] = v }
    return o
}

/**
 * A recorded voice message's waveform (signal `playing`) with drag-to-seek: the chat-bubble
 * control.
 *
 * ```kotlin
 * SinuaVoiceMessage(envelope = peaks, progress = position, onSeek = { player.seekTo(it) },
 *     modifier = Modifier.size(220.dp, 40.dp))
 * ```
 *
 * Sinua decodes no audio: pass the clip's loudness as [envelope] (up to 64 values, 0..1) and
 * the position as [progress]. A drag or a tap reports the position under the finger through
 * [onSeek], on the same row of bars the engine draws. TalkBack: an adjustable progress.
 */
@Composable
fun SinuaVoiceMessage(
    envelope: List<Double>,
    progress: Double,
    modifier: Modifier = Modifier,
    label: String = "Voice message",
    overrides: Map<String, Double> = emptyMap(),
    theme: FxTheme = FxTheme.AUTO,
    onSeek: ((Double) -> Unit)? = null,
) {
    val keys = remember(envelope, progress, overrides) { sinuaVoiceMessageKeys(envelope, progress, overrides) }
    val seek by rememberUpdatedState(onSeek)
    val gestures = if (onSeek == null) {
        Modifier
    } else {
        Modifier
            .pointerInput(Unit) { detectTapGestures { seek?.invoke(sinuaVoiceMessageSeek(size, it.x)) } }
            .pointerInput(Unit) {
                detectHorizontalDragGestures { change, _ ->
                    seek?.invoke(sinuaVoiceMessageSeek(size, change.position.x))
                }
            }
    }
    val a11y = Modifier.clearAndSetSemantics {
        contentDescription = label
        progressBarRangeInfo = ProgressBarRangeInfo(progress.coerceIn(0.0, 1.0).toFloat(), 0f..1f)
        if (onSeek != null) {
            setProgress { v: Float ->
                seek?.invoke(v.toDouble().coerceIn(0.0, 1.0))
                true
            }
        }
    }
    SinuaView(
        pattern = "playing",
        modifier = modifier.then(gestures).then(a11y),
        overrides = keys,
        theme = theme,
        contentDescription = "",
    )
}
