package dev.sinua.view

import androidx.compose.foundation.Canvas
import androidx.compose.foundation.background
import androidx.compose.foundation.gestures.awaitEachGesture
import androidx.compose.foundation.gestures.awaitFirstDown
import androidx.compose.foundation.gestures.waitForUpOrCancellation
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.offset
import androidx.compose.foundation.layout.requiredSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.DisposableEffect
import androidx.compose.runtime.getValue
import androidx.compose.runtime.mutableStateOf
import androidx.compose.runtime.remember
import androidx.compose.runtime.rememberUpdatedState
import androidx.compose.runtime.setValue
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.graphics.StrokeCap
import androidx.compose.ui.graphics.StrokeJoin
import androidx.compose.ui.graphics.drawscope.Stroke
import androidx.compose.ui.graphics.drawscope.scale
import androidx.compose.ui.graphics.vector.PathParser
import androidx.compose.ui.input.pointer.PointerInputChange
import androidx.compose.ui.input.pointer.pointerInput
import androidx.compose.ui.semantics.CustomAccessibilityAction
import androidx.compose.ui.semantics.Role
import androidx.compose.ui.semantics.clearAndSetSemantics
import androidx.compose.ui.semantics.contentDescription
import androidx.compose.ui.semantics.customActions
import androidx.compose.ui.semantics.onClick
import androidx.compose.ui.semantics.role
import androidx.compose.ui.semantics.selected
import androidx.compose.ui.semantics.semantics
import androidx.compose.ui.unit.Dp
import androidx.compose.ui.unit.dp
import dev.sinua.voice.VoiceButtonController
import dev.sinua.voice.VoiceButtonLabels
import dev.sinua.voice.VoiceButtonMode
import dev.sinua.voice.VoiceButtonState
import dev.sinua.voice.VoiceOverridesOptions
import dev.sinua.voice.VoiceSource
import dev.sinua.voice.voiceButtonHint

/** Hold this long to end the session (also the ✕ and the "End voice session" action). */
const val VOICE_BUTTON_LONG_PRESS_MS = 600L

/**
 * A mic button bound to a voice source (docs/fx-view.md, *Voice button*): ready, connecting,
 * listening, muted or error, derived from the source. The logic is `VoiceButtonController`
 * (the same state table as Web and iOS); the ring behind the icon is an ordinary engine view.
 *
 * ```kotlin
 * SinuaView(pattern = "glowing", voice = source)
 * SinuaVoiceButton(source)                                   // tap: connect, then mute / unmute
 * SinuaVoiceButton(source, mode = VoiceButtonMode.PUSH_TO_TALK)  // hold to talk
 * ```
 * Give the view and the button the same source: both go through its `SharedVoiceSource`, so
 * neither steals the other's callbacks. A long press or the small ✕ ends the session.
 */
@Composable
fun SinuaVoiceButton(
    source: VoiceSource,
    modifier: Modifier = Modifier,
    mode: VoiceButtonMode = VoiceButtonMode.TOGGLE,
    labels: VoiceButtonLabels = VoiceButtonLabels(),
    size: Dp = 56.dp,
    onChange: ((VoiceButtonState, String?) -> Unit)? = null,
) {
    val controller = remember(source) { VoiceButtonController(source, mode) }
    controller.mode = mode
    // The ring's tracker: a stronger pulse than a view's default (0.18 barely moves a ring this small).
    val ringVoice = remember(controller) { controller.source.track(VoiceOverridesOptions(audioStrength = 0.75)) }
    var state by remember(controller) { mutableStateOf(controller.state) }
    var reason by remember(controller) { mutableStateOf(controller.reason) }
    val change by rememberUpdatedState(onChange)
    DisposableEffect(controller) {
        val off = controller.onChange { m ->
            state = m.state
            reason = m.reason
            change?.invoke(m.state, m.reason)
        }
        onDispose {
            off()
            ringVoice.release()
            controller.destroy()
        }
    }

    val dark = isSystemInDarkTheme()
    val bg = if (dark) Color(0xFF26262C) else Color(0xFFF1F1F4)
    val fg = if (dark) Color(0xFFECECF0) else Color(0xFF1C1C21)
    val tint = when (state) {
        VoiceButtonState.ERROR -> if (dark) Color(0xFFEF6A6A) else Color(0xFFC93B3B)
        VoiceButtonState.MUTED -> if (dark) Color(0xFF8D8D96) else Color(0xFF8A8A93)
        else -> fg
    }
    val live = state == VoiceButtonState.CONNECTING || state == VoiceButtonState.LISTENING ||
        state == VoiceButtonState.MUTED
    val label = if (state == VoiceButtonState.ERROR &&
        reason != null
    ) {
        "${labels.error}: $reason"
    } else {
        labels.label(state)
    }
    val hint = voiceButtonHint(state, controller.mode, controller.canMute)

    Box(modifier.size(size)) {
        Box(
            Modifier
                .size(size)
                .clip(CircleShape)
                .background(bg)
                .pointerInput(controller) {
                    awaitEachGesture {
                        awaitFirstDown()
                        val ptt = controller.mode == VoiceButtonMode.PUSH_TO_TALK
                        if (ptt) controller.press()
                        var up: PointerInputChange? = null
                        val lifted = withTimeoutOrNull(VOICE_BUTTON_LONG_PRESS_MS) {
                            up = waitForUpOrCancellation()
                            true
                        }
                        val s = controller.state
                        if (lifted == null && s != VoiceButtonState.READY && s != VoiceButtonState.ERROR) {
                            // A long press on a live session ends it; the lift that follows does nothing.
                            controller.end()
                            waitForUpOrCancellation()
                            return@awaitEachGesture
                        }
                        if (lifted == null) up = waitForUpOrCancellation()
                        if (ptt) {
                            controller.release()
                        } else if (up != null) {
                            controller.press()
                        }
                    }
                }
                .clearAndSetSemantics {
                    role = Role.Button
                    contentDescription = label
                    selected = state == VoiceButtonState.MUTED
                    onClick(label = hint.ifEmpty { null }) {
                        controller.assistiveActivate()
                        true
                    }
                    if (live) {
                        customActions = listOf(
                            CustomAccessibilityAction(labels.end) {
                                controller.end()
                                true
                            },
                        )
                    }
                },
            contentAlignment = Alignment.Center,
        ) {
            voiceButtonRing(state)?.let { ring ->
                // The ring sits on the button's edge and swells outward with the level.
                SinuaView(
                    pattern = ring.pattern,
                    modifier = Modifier.requiredSize(size * 1.28f),
                    overrides = ring.overrides,
                    speed = ring.speed,
                    state = ring.state,
                    voiceOverrides = ringVoice.overrides,
                    contentDescription = "",
                )
            }
            Canvas(Modifier.size(size * 0.44f)) {
                val icon = when (state) {
                    VoiceButtonState.ERROR -> ICON_ALERT
                    VoiceButtonState.MUTED -> ICON_MIC_OFF
                    else -> ICON_MIC
                }
                scale(this.size.width / 24f, this.size.height / 24f, pivot = androidx.compose.ui.geometry.Offset.Zero) {
                    drawPath(icon, tint, style = Stroke(width = 1.8f, cap = StrokeCap.Round, join = StrokeJoin.Round))
                }
            }
        }
        if (live) {
            Box(
                Modifier
                    .align(Alignment.TopEnd)
                    .offset(x = 6.dp, y = (-6).dp)
                    .size(22.dp)
                    .clip(CircleShape)
                    .background(bg)
                    .pointerInput(controller) {
                        awaitEachGesture {
                            awaitFirstDown()
                            if (waitForUpOrCancellation() != null) controller.end()
                        }
                    }
                    .semantics {
                        role = Role.Button
                        contentDescription = labels.end
                        onClick {
                            controller.end()
                            true
                        }
                    },
                contentAlignment = Alignment.Center,
            ) {
                Canvas(Modifier.size(12.dp)) {
                    scale(
                        this.size.width / 24f,
                        this.size.height / 24f,
                        pivot = androidx.compose.ui.geometry.Offset.Zero,
                    ) {
                        drawPath(ICON_CLOSE, fg, style = Stroke(width = 2.4f, cap = StrokeCap.Round))
                    }
                }
            }
        }
    }
}

/** The inner ring for a state (Web's `voiceButtonRing`): an engine view, or null for the icon only. */
internal data class VoiceButtonRing(
    val pattern: String,
    val state: String,
    val overrides: Map<String, Double>,
    val speed: Double,
)

internal fun voiceButtonRing(s: VoiceButtonState): VoiceButtonRing? {
    val ring = mapOf("progress" to 1.0, "strokeWidth" to 0.045)
    return when (s) {
        VoiceButtonState.CONNECTING ->
            VoiceButtonRing("loading", "initializing", mapOf("strokeWidth" to 0.045, "trackOpacity" to 0.15), 1.0)

        VoiceButtonState.LISTENING -> VoiceButtonRing("completing", "listening", ring, 1.0)

        VoiceButtonState.MUTED -> VoiceButtonRing("completing", "listening", ring, 0.15)

        else -> null
    }
}

// The web element's icons (voice-button.ts), 24-unit paths.
private val ICON_MIC =
    PathParser().parsePathString("M9 6a3 3 0 0 1 6 0v5a3 3 0 0 1-6 0zM5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21").toPath()
private val ICON_MIC_OFF =
    PathParser().parsePathString("M9 6a3 3 0 0 1 6 0v5a3 3 0 0 1-6 0zM5.5 11a6.5 6.5 0 0 0 13 0M12 17.5V21M4 4l16 16")
        .toPath()
private val ICON_ALERT =
    PathParser().parsePathString("M12 3.5a8.5 8.5 0 1 0 0 17a8.5 8.5 0 1 0 0-17zM12 7.5v5.5M12 16.2v.3").toPath()
private val ICON_CLOSE = PathParser().parsePathString("M6 6l12 12M18 6L6 18").toPath()
