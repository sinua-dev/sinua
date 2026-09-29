package dev.sinua.voice

import java.util.WeakHashMap

/**
 * One source, many listeners -- the Kotlin mirror of `@sinua/core`'s `SharedVoiceSource`
 * (docs/audio-pipeline.md, *Sharing a source*). A [VoiceSource] holds ONE callback of each
 * kind, so a second subscriber (a view next to a voice button, two views) silently replaces
 * the first. This subscribes once and fans out; it is itself a [VoiceSource], and it owns the
 * session's mute. Main thread only; callbacks arrive there.
 */
class SharedVoiceSource private constructor(
    /** The wrapped source. Don't subscribe to it directly: that takes its one callback away from this fan-out. */
    val source: VoiceSource,
) : VoiceSource {
    private val metricsCbs = LinkedHashSet<(VoiceMetrics) -> Unit>()
    private val stateCbs = LinkedHashSet<(AgentState) -> Unit>()
    private val interruptCbs = LinkedHashSet<() -> Unit>()
    private val muteCbs = LinkedHashSet<(Boolean) -> Unit>()
    private val connectionCbs = LinkedHashSet<(Boolean) -> Unit>()
    private val errorCbs = LinkedHashSet<(Throwable) -> Unit>()

    /** The source's last reported state ([AgentState.IDLE] before any). */
    var state: AgentState = AgentState.IDLE
        private set
    var muted = false
        private set

    /** The last [onConnectionChange] value (false before any). */
    var connected = false
        private set

    init {
        source.onMetrics { m -> metricsCbs.toList().forEach { it(m) } }
        source.onStateChange { s ->
            state = s
            stateCbs.toList().forEach { it(s) }
        }
        source.onInterrupt { interruptCbs.toList().forEach { it() } }
        source.onConnectionChange { c ->
            connected = c
            connectionCbs.toList().forEach { it(c) }
        }
        source.onError { e -> errorCbs.toList().forEach { it(e) } }
    }

    private fun <T> add(set: MutableSet<T>, cb: T): () -> Unit {
        set.add(cb)
        return { set.remove(cb) }
    }

    // --- listeners (each returns its cancel) ---

    fun listenMetrics(cb: (VoiceMetrics) -> Unit): () -> Unit = add(metricsCbs, cb)

    fun listenState(cb: (AgentState) -> Unit): () -> Unit = add(stateCbs, cb)

    fun listenInterrupt(cb: () -> Unit): () -> Unit = add(interruptCbs, cb)

    /** Called with the new value whenever [setMuted] changes it. */
    fun listenMute(cb: (Boolean) -> Unit): () -> Unit = add(muteCbs, cb)

    /** Only fires for a source that reports it ([reportsConnection]). */
    fun listenConnection(cb: (Boolean) -> Unit): () -> Unit = add(connectionCbs, cb)

    fun listenError(cb: (Throwable) -> Unit): () -> Unit = add(errorCbs, cb)

    // --- VoiceSource (the `on…` forms add a listener you can't remove) ---

    override fun onMetrics(cb: (VoiceMetrics) -> Unit) {
        listenMetrics(cb)
    }

    override fun onStateChange(cb: (AgentState) -> Unit) {
        listenState(cb)
    }

    override fun onInterrupt(cb: () -> Unit) {
        listenInterrupt(cb)
    }

    override fun onConnectionChange(cb: (Boolean) -> Unit) {
        listenConnection(cb)
    }

    override fun onError(cb: (Throwable) -> Unit) {
        listenError(cb)
    }

    override val supportsMute: Boolean get() = source.supportsMute
    override val reportsConnection: Boolean get() = source.reportsConnection

    /** Connects unmuted: a new session never starts silent from an old mute. */
    override fun connect() {
        setMuted(false)
        source.connect()
    }

    override fun disconnect() {
        source.disconnect()
    }

    /**
     * Mutes or unmutes the microphone: silence goes out, the session stays up, and views bound
     * through this fan-out show the muted cue. Always unmuted when the source can't mute.
     */
    override fun setMuted(muted: Boolean) {
        val m = muted && source.supportsMute
        source.setMuted(m)
        if (m == this.muted) return
        this.muted = m
        muteCbs.toList().forEach { it(m) }
    }

    /**
     * A view's own tracker fed from this fan-out: its family's easing and history, the muted
     * cue following [muted]. [Tracked.release] unsubscribes it.
     */
    fun track(options: VoiceOverridesOptions = VoiceOverridesOptions()): Tracked {
        val v = VoiceOverrides(options)
        v.setState(state)
        v.muted = muted
        val offs = listOf(
            listenMetrics { v.push(it) },
            listenInterrupt { v.interrupt() },
            listenState {
                v.setState(it)
                if (it == AgentState.IDLE) v.reset()
            },
            listenMute { v.muted = it },
        )
        return Tracked(v, this, offs)
    }

    /** One view's subscription: its [VoiceOverrides], until [release]. */
    class Tracked internal constructor(
        val overrides: VoiceOverrides,
        /** Kept so the fan-out lives while a view uses it. */
        val source: SharedVoiceSource,
        private var offs: List<() -> Unit>,
    ) {
        fun release() {
            offs.forEach { it() }
            offs = emptyList()
        }
    }

    companion object {
        // Weak keys; the fan-out holds its source, so values are weak too (a fan-out
        // lives as long as a view or a button holds it).
        private val table = WeakHashMap<VoiceSource, java.lang.ref.WeakReference<SharedVoiceSource>>()

        /**
         * The fan-out for [source]: the same instance every time for the same source (while
         * something holds it), so a view and a voice button given one raw source share one
         * subscription. A [SharedVoiceSource] is returned as is.
         */
        @JvmStatic
        fun of(source: VoiceSource): SharedVoiceSource {
            if (source is SharedVoiceSource) return source
            table[source]?.get()?.let { return it }
            return SharedVoiceSource(source).also { table[source] = java.lang.ref.WeakReference(it) }
        }
    }
}
