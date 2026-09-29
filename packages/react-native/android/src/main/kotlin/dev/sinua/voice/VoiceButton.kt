package dev.sinua.voice

/*
 * The voice button's logic -- the Kotlin mirror of `@sinua/core`'s voiceButton.ts
 * (docs/fx-view.md, *Voice button*). A pure state machine plus a controller that runs its
 * effects on a source. spec/voice-button-cases.json holds all three platforms to the same
 * table. The Compose control is `SinuaVoiceButton` in dev.sinua.view.
 */

/** [TOGGLE]: a press connects, then mutes and unmutes. [PUSH_TO_TALK]: hold to talk. */
enum class VoiceButtonMode(val wire: String) {
    TOGGLE("toggle"),
    PUSH_TO_TALK("pushToTalk"),
    ;

    companion object {
        fun fromWire(s: String): VoiceButtonMode? = entries.firstOrNull { it.wire == s }
    }
}

enum class VoiceButtonState(val wire: String) {
    READY("ready"),
    CONNECTING("connecting"),
    LISTENING("listening"),
    MUTED("muted"),
    ERROR("error"),
}

sealed interface VoiceButtonEvent {
    data object Press : VoiceButtonEvent

    data object Release : VoiceButtonEvent

    /** A long-press or the ✕: end the session. */
    data object End : VoiceButtonEvent

    data object ConnectOk : VoiceButtonEvent

    data class ConnectFail(val reason: String) : VoiceButtonEvent

    /** The session ended on the source's side (a remote hang-up, a drop it gave up on). */
    data object Dropped : VoiceButtonEvent

    /** The mute changed elsewhere. */
    data class MuteChanged(val muted: Boolean) : VoiceButtonEvent
}

enum class VoiceButtonEffect(val wire: String) {
    CONNECT("connect"),
    DISCONNECT("disconnect"),
    MUTE("mute"),
    UNMUTE("unmute"),
}

data class VoiceButtonModel(
    val state: VoiceButtonState = VoiceButtonState.READY,
    /** The last failure, for [VoiceButtonState.ERROR]. */
    val reason: String? = null,
    /** Push-to-talk: held down right now. */
    val held: Boolean = false,
)

/** One event in, the next model and the effects to run on the source, in order. */
fun voiceButtonStep(
    m: VoiceButtonModel,
    e: VoiceButtonEvent,
    mode: VoiceButtonMode,
    canMute: Boolean,
): Pair<VoiceButtonModel, List<VoiceButtonEffect>> {
    fun to(s: VoiceButtonState, fx: List<VoiceButtonEffect> = emptyList(), held: Boolean? = null) =
        m.copy(state = s, reason = if (s == VoiceButtonState.ERROR) m.reason else null, held = held ?: m.held) to fx
    val same = m to emptyList<VoiceButtonEffect>()
    val ptt = mode == VoiceButtonMode.PUSH_TO_TALK && canMute
    val live = m.state == VoiceButtonState.LISTENING || m.state == VoiceButtonState.MUTED
    val connect = listOf(VoiceButtonEffect.CONNECT)
    return when (e) {
        VoiceButtonEvent.Press -> when {
            m.state == VoiceButtonState.READY || m.state == VoiceButtonState.ERROR ->
                to(VoiceButtonState.CONNECTING, connect, held = ptt)

            m.state == VoiceButtonState.CONNECTING -> if (ptt) to(VoiceButtonState.CONNECTING, held = true) else same

            ptt ->
                if (m.state == VoiceButtonState.MUTED) {
                    to(VoiceButtonState.LISTENING, listOf(VoiceButtonEffect.UNMUTE), held = true)
                } else {
                    to(VoiceButtonState.LISTENING, held = true)
                }

            !canMute -> to(VoiceButtonState.READY, listOf(VoiceButtonEffect.DISCONNECT))

            m.state == VoiceButtonState.LISTENING -> to(VoiceButtonState.MUTED, listOf(VoiceButtonEffect.MUTE))

            else -> to(VoiceButtonState.LISTENING, listOf(VoiceButtonEffect.UNMUTE))
        }

        VoiceButtonEvent.Release -> when {
            !ptt || !m.held -> same

            m.state == VoiceButtonState.LISTENING -> to(
                VoiceButtonState.MUTED,
                listOf(VoiceButtonEffect.MUTE),
                held = false,
            )

            else -> m.copy(held = false) to emptyList()
        }

        VoiceButtonEvent.End -> when {
            m.state == VoiceButtonState.CONNECTING || live ->
                to(VoiceButtonState.READY, listOf(VoiceButtonEffect.DISCONNECT), held = false)

            m.state == VoiceButtonState.ERROR -> to(VoiceButtonState.READY, held = false)

            else -> same
        }

        VoiceButtonEvent.ConnectOk -> when {
            m.state != VoiceButtonState.CONNECTING -> same

            // Push-to-talk released before the session was up: it starts muted.
            ptt && !m.held -> to(VoiceButtonState.MUTED, listOf(VoiceButtonEffect.MUTE))

            else -> to(VoiceButtonState.LISTENING)
        }

        is VoiceButtonEvent.ConnectFail ->
            if (m.state !=
                VoiceButtonState.CONNECTING
            ) {
                same
            } else {
                VoiceButtonModel(VoiceButtonState.ERROR, e.reason) to emptyList()
            }

        VoiceButtonEvent.Dropped -> if (live) to(VoiceButtonState.READY, held = false) else same

        is VoiceButtonEvent.MuteChanged ->
            if (!live) same else to(if (e.muted) VoiceButtonState.MUTED else VoiceButtonState.LISTENING)
    }
}

/** The accessible name per state; the same default wording on every platform. */
data class VoiceButtonLabels(
    val ready: String = "Start voice",
    val connecting: String = "Connecting",
    val listening: String = "Microphone on",
    val muted: String = "Microphone muted",
    val error: String = "Voice unavailable",
    /** The ✕ / long-press action. */
    val end: String = "End voice session",
) {
    fun label(s: VoiceButtonState): String = when (s) {
        VoiceButtonState.READY -> ready
        VoiceButtonState.CONNECTING -> connecting
        VoiceButtonState.LISTENING -> listening
        VoiceButtonState.MUTED -> muted
        VoiceButtonState.ERROR -> error
    }
}

/** What a screen reader should hear as the action (the hint), per state and mode. */
fun voiceButtonHint(s: VoiceButtonState, mode: VoiceButtonMode, canMute: Boolean): String = when (s) {
    VoiceButtonState.READY -> "Connects the voice session"

    VoiceButtonState.CONNECTING -> ""

    VoiceButtonState.ERROR -> "Tries again"

    VoiceButtonState.LISTENING -> when {
        mode == VoiceButtonMode.PUSH_TO_TALK && canMute -> "Release to mute"
        canMute -> "Mutes the microphone"
        else -> "Ends the voice session"
    }

    VoiceButtonState.MUTED -> if (mode == VoiceButtonMode.PUSH_TO_TALK) "Hold to talk" else "Unmutes the microphone"
}

/**
 * Runs the state machine against a source: presses in, `connect` / `disconnect` /
 * `setMuted` out, the source's drops, failures and mutes folded back in. Main thread only.
 *
 * On Android a vendor's `connect()` returns while the session is still opening, so a source
 * that [VoiceSource.reportsConnection] counts as connected on `onConnectionChange(true)`, and
 * its `onError` while connecting is the failure; any other source is connected when
 * `connect()` returns.
 */
class VoiceButtonController(source: VoiceSource, var mode: VoiceButtonMode = VoiceButtonMode.TOGGLE) {
    val source: SharedVoiceSource = SharedVoiceSource.of(source)
    var model = VoiceButtonModel()
        private set
    private val changeCbs = LinkedHashSet<(VoiceButtonModel) -> Unit>()
    private val offs: List<() -> Unit>

    init {
        val s = this.source
        offs = listOf(
            if (s.reportsConnection) {
                s.listenConnection { up ->
                    dispatch(if (up) VoiceButtonEvent.ConnectOk else VoiceButtonEvent.Dropped)
                }
            } else {
                s.listenState { if (it == AgentState.IDLE) dispatch(VoiceButtonEvent.Dropped) }
            },
            s.listenError { e -> dispatch(VoiceButtonEvent.ConnectFail(message(e))) },
            s.listenMute { dispatch(VoiceButtonEvent.MuteChanged(it)) },
        )
    }

    val state: VoiceButtonState get() = model.state
    val reason: String? get() = model.reason
    val canMute: Boolean get() = source.supportsMute

    /** Called on every model change; returns its cancel. */
    fun onChange(cb: (VoiceButtonModel) -> Unit): () -> Unit {
        changeCbs.add(cb)
        return { changeCbs.remove(cb) }
    }

    fun press() = dispatch(VoiceButtonEvent.Press)

    fun release() = dispatch(VoiceButtonEvent.Release)

    fun end() = dispatch(VoiceButtonEvent.End)

    /**
     * A screen reader's activation in push-to-talk: it can't hold, so it toggles instead (and
     * connecting starts live).
     */
    fun assistiveActivate() {
        if (mode == VoiceButtonMode.TOGGLE || state == VoiceButtonState.READY || state == VoiceButtonState.ERROR) {
            return press()
        }
        val m = mode
        mode = VoiceButtonMode.TOGGLE
        press()
        mode = m
    }

    /** Unsubscribes from the source. Doesn't disconnect it: the app owns the session. */
    fun destroy() {
        offs.forEach { it() }
        changeCbs.clear()
    }

    internal fun dispatch(e: VoiceButtonEvent) {
        val (next, effects) = voiceButtonStep(model, e, mode, source.supportsMute)
        val changed = next != model
        model = next
        if (changed) changeCbs.toList().forEach { it(next) }
        effects.forEach { run(it) }
    }

    private fun run(fx: VoiceButtonEffect) {
        when (fx) {
            VoiceButtonEffect.CONNECT -> {
                try {
                    source.connect()
                } catch (e: Throwable) {
                    dispatch(VoiceButtonEvent.ConnectFail(message(e)))
                    return
                }
                if (!source.reportsConnection) dispatch(VoiceButtonEvent.ConnectOk)
            }

            VoiceButtonEffect.DISCONNECT -> source.disconnect()

            VoiceButtonEffect.MUTE -> source.setMuted(true)

            VoiceButtonEffect.UNMUTE -> source.setMuted(false)
        }
    }

    private companion object {
        fun message(e: Throwable): String = e.message ?: e.javaClass.simpleName
    }
}
