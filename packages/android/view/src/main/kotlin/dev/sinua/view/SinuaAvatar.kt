package dev.sinua.view

import androidx.compose.foundation.Image
import androidx.compose.foundation.layout.Box
import androidx.compose.foundation.layout.BoxWithConstraints
import androidx.compose.foundation.layout.aspectRatio
import androidx.compose.foundation.layout.fillMaxSize
import androidx.compose.foundation.layout.size
import androidx.compose.foundation.shape.CircleShape
import androidx.compose.runtime.Composable
import androidx.compose.runtime.remember
import androidx.compose.ui.Alignment
import androidx.compose.ui.Modifier
import androidx.compose.ui.draw.clip
import androidx.compose.ui.graphics.painter.Painter
import androidx.compose.ui.layout.ContentScale
import androidx.compose.ui.unit.Dp
import dev.sinua.voice.VoiceSource

/** Default image radius: the ring `talking`'s own `innerRadius` default. */
const val SINUA_AVATAR_INNER_RADIUS = 0.34

/** The image's diameter in a view [side] across: `2 × innerRadius`, the ring's own inner edge. */
fun sinuaAvatarImageDiameter(side: Dp, innerRadius: Double): Dp = side * (2 * innerRadius).toFloat()

/**
 * An image in a circle with the ring `talking` round it: the "who is speaking" indicator
 * for call grids, agent lists and chat headers.
 *
 * ```kotlin
 * SinuaAvatar(painterResource(R.drawable.ada), label = "Ada", voice = source, modifier = Modifier.size(56.dp))
 * ```
 *
 * The image's diameter is `2 × innerRadius` of the (square) view, the same `innerRadius`
 * the ring draws round, so the two can't drift apart. The ring is decorative; [label]
 * names the avatar for TalkBack.
 */
@Composable
fun SinuaAvatar(
    painter: Painter,
    label: String,
    modifier: Modifier = Modifier,
    innerRadius: Double = SINUA_AVATAR_INNER_RADIUS,
    overrides: Map<String, Double> = emptyMap(),
    state: String? = null,
    voice: VoiceSource? = null,
    theme: FxTheme = FxTheme.AUTO,
    paused: Boolean = false,
) {
    val r = innerRadius.coerceIn(0.1, 0.44)
    val ring = remember(overrides, r) { overrides + ("innerRadius" to r) }
    BoxWithConstraints(modifier.aspectRatio(1f), contentAlignment = Alignment.Center) {
        val side = if (maxWidth < maxHeight) maxWidth else maxHeight
        Image(
            painter = painter,
            contentDescription = label,
            contentScale = ContentScale.Crop,
            modifier = Modifier.size(sinuaAvatarImageDiameter(side, r)).clip(CircleShape),
        )
        Box(Modifier.fillMaxSize()) {
            SinuaView(
                pattern = "talking",
                modifier = Modifier.fillMaxSize(),
                overrides = ring,
                state = state,
                voice = voice,
                theme = theme,
                paused = paused,
                contentDescription = "",
            )
        }
    }
}
