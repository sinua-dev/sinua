package dev.sinua.voice

import org.json.JSONObject
import uniffi.core_engine.ConversationFrame
import uniffi.core_engine.conversationAt
import uniffi.core_engine.conversationSample
import uniffi.core_engine.conversationSampleNames

/**
 * A simulated conversation as a [VoiceSource] -- the Kotlin mirror of `@sinua/core`'s
 * `SimulatedVoiceSource` (docs/audio-pipeline.md, *Simulated conversations*). A script of
 * turns plays like a real agent: state changes on a timeline, a speech-like level and bands
 * while someone talks, the barge-in flash. No microphone, no AudioRecord, no network, no
 * permission. The curves come from the engine ([conversationAt]), so Web and iOS play the
 * same conversation.
 *
 * ```kotlin
 * val voice = SimulatedVoiceSource.sample("barge-in")
 * SinuaView(pattern = "glowing", voice = voice)
 * voice.connect()   // plays and loops
 * ```
 * Call on the main thread; callbacks arrive there.
 */
class SimulatedVoiceSource(
    /** A script as JSON text (`{ turns: [{ state, seconds, voice?, bargeIn?, line? }] }`). */
    private val script: String,
    bands: Int = 16,
    private val rate: Double = 30.0,
    loop: Boolean? = null,
    private val main: MainDispatcher = LooperMainDispatcher(),
    /** `false` = no timer; drive time with [advance] (tests, a loop that already ticks). */
    private val autoTick: Boolean = true,
) : VoiceSource {
    /** One turn on the timeline, for a timeline UI. */
    data class Turn(
        val state: String,
        val start: Double,
        val seconds: Double,
        val line: String,
        val bargeIn: Boolean,
        /** `"user"`, `"agent"`, or null for a silent turn. */
        val voice: String? = null,
    )

    private val bandCount = maxOf(1, bands).toUInt()

    /** Whether time wraps at the end. */
    var loop: Boolean
    val turns: List<Turn>

    /** The script's length in seconds. */
    val duration: Double

    /** The script time now, seconds. */
    var time = 0.0
        private set
    var isPlaying = false
        private set

    private var connected = false
    private var lastTick = 0L
    private var lastState: String? = null
    private var lastTurn = -1
    private var metricsCb: ((VoiceMetrics) -> Unit)? = null
    private var stateCb: ((AgentState) -> Unit)? = null
    private var interruptCb: (() -> Unit)? = null
    private var frameCb: ((ConversationFrame) -> Unit)? = null
    private var transcriptCb: ((TranscriptUpdate) -> Unit)? = null

    /** The turn being transcribed (`turn` is the script's index) and the counters that name turns. */
    private class Said(val turn: Int, val id: String, val role: TranscriptRole, var text: String)
    private var said: Said? = null
    private var userTurns = 0
    private var assistantTurns = 0
    private var connectionCb: ((Boolean) -> Unit)? = null
    private var muted = false

    private val ticker = object : Runnable {
        override fun run() {
            if (!isPlaying) return
            val now = System.nanoTime()
            advance((now - lastTick) / 1e9)
            lastTick = now
            main.postDelayed(this, (1000 / rate).toLong())
        }
    }

    init {
        val probe = conversationAt(script, 0.0, 1u)
        if (!probe.ok) {
            val d = probe.diagnostics.firstOrNull { it.severity == "error" }
            throw IllegalArgumentException(
                "SimulatedVoiceSource: " + (d?.let { "${it.path.ifEmpty { "/" }}: ${it.message}" } ?: "not a script"),
            )
        }
        val doc = JSONObject(script)
        this.loop = loop ?: doc.optBoolean("loop", false)
        val arr = doc.getJSONArray("turns")
        var start = 0.0
        turns = (0 until arr.length()).map { i ->
            val t = arr.getJSONObject(i)
            val seconds = t.getDouble("seconds")
            Turn(
                t.getString("state"),
                start,
                seconds,
                t.optString("line", ""),
                t.optBoolean("bargeIn", false),
                t.optString("voice", "").ifEmpty { null },
            )
                .also { start += seconds }
        }
        duration = probe.total
    }

    override fun onMetrics(cb: (VoiceMetrics) -> Unit) {
        metricsCb = cb
    }

    override fun onStateChange(cb: (AgentState) -> Unit) {
        stateCb = cb
    }

    override fun onInterrupt(cb: () -> Unit) {
        interruptCb = cb
    }

    override val supportsMute: Boolean get() = true
    override val reportsConnection: Boolean get() = true

    override fun onConnectionChange(cb: (Boolean) -> Unit) {
        connectionCb = cb
    }

    /** Muted, the user's turns go silent (as a muted mic would); the agent's keep playing. */
    override fun setMuted(muted: Boolean) {
        this.muted = muted
        emit()
    }

    /** Every tick's full reading (turn, progress, the line said so far), for captions and a timeline. */
    fun onFrame(cb: (ConversationFrame) -> Unit) {
        frameCb = cb
    }

    /**
     * Transcript updates: each turn's line as it is said ([TranscriptTiming.SYNCED]); a turn cut
     * by a barge-in ends `truncated`. Muted user turns say nothing. Main thread.
     */
    override fun onTranscript(cb: (TranscriptUpdate) -> Unit) {
        transcriptCb = cb
    }

    override val supportsTranscript: Boolean get() = true
    override val transcriptTiming: TranscriptTiming get() = TranscriptTiming.SYNCED

    /** Starts playing from the current time. Never touches the microphone. */
    override fun connect() {
        val was = connected
        connected = true
        if (!was) connectionCb?.invoke(true)
        lastState = null
        lastTurn = -1
        play()
        emit()
    }

    override fun disconnect() {
        close(cut = false)
        val was = connected
        connected = false
        if (was) connectionCb?.invoke(false)
        pause()
        metricsCb?.invoke(VoiceMetrics(0.0, List(bandCount.toInt()) { 0.0 }))
        if (lastState != "idle") stateCb?.invoke(AgentState.IDLE)
        lastState = "idle"
    }

    fun play() {
        if (!connected || isPlaying) return
        isPlaying = true
        if (autoTick) {
            lastTick = System.nanoTime()
            main.post(ticker)
        }
    }

    fun pause() {
        isPlaying = false
        main.remove(ticker)
    }

    /** Jump to [t] seconds (no barge-in flash for a turn you jump into). */
    fun seek(t: Double) {
        time = t.coerceIn(0.0, duration)
        lastTurn = conversationAt(script, minOf(time, duration), 1u).turn.toInt()
        emit()
    }

    /** Move time on by [dt] seconds (the timer calls this; so can tests). */
    fun advance(dt: Double) {
        if (!isPlaying) return
        time += maxOf(0.0, dt)
        if (time >= duration) time = if (loop) time % duration else duration
        emit()
    }

    private fun emit() {
        if (!connected) return
        // `loop` is ours, not the script's: past the end without it, hold the last instant.
        val at = if (loop) time else minOf(time, duration - 1e-9)
        val f = conversationAt(script, at, bandCount)
        if (f.turn.toInt() != lastTurn) {
            val entering = lastTurn != -1 && f.bargeIn
            lastTurn = f.turn.toInt()
            if (entering) interruptCb?.invoke()
            // The turn that was being said ends: cut short by a barge-in, or said in full.
            val s = said
            if (s != null && s.turn != f.turn.toInt()) close(cut = entering)
        }
        transcribe(f)
        if (f.state != lastState) {
            lastState = f.state
            stateCb?.invoke(AgentState.fromWire(f.state) ?: AgentState.IDLE)
        }
        if (muted && turns.getOrNull(f.turn.toInt())?.voice == "user") {
            metricsCb?.invoke(VoiceMetrics(0.0, List(f.bands.size) { 0.0 }))
        } else {
            metricsCb?.invoke(VoiceMetrics(f.level, f.bands))
        }
        frameCb?.invoke(f)
    }

    /** Speaker of a turn: its `voice`, else what its state implies (listening = the user). */
    private fun roleOf(turn: Int, state: String): TranscriptRole? = when (turns.getOrNull(turn)?.voice) {
        "user" -> TranscriptRole.USER

        "agent" -> TranscriptRole.ASSISTANT

        else -> when (state) {
            "speaking" -> TranscriptRole.ASSISTANT
            "listening" -> TranscriptRole.USER
            else -> null
        }
    }

    private fun transcribe(f: ConversationFrame) {
        val cb = transcriptCb ?: return
        val role = roleOf(f.turn.toInt(), f.state) ?: return
        if (f.line.isEmpty() || (role == TranscriptRole.USER && muted)) return
        val line = java.text.Normalizer.normalize(f.line, java.text.Normalizer.Form.NFC)
        val n = minOf(f.shown.toInt(), line.codePointCount(0, line.length))
        val text = line.substring(0, line.offsetByCodePoints(0, n))
        val s = said ?: run {
            if (text.isEmpty()) return
            val id = if (role == TranscriptRole.USER) "u${++userTurns}" else "a${++assistantTurns}"
            Said(f.turn.toInt(), id, role, "").also { said = it }
        }
        if (text == s.text) return
        s.text = text
        cb(TranscriptUpdate(role, text, false, s.id))
    }

    /** Ends the turn being said: `cut` by a barge-in (what was said so far), else in full. */
    private fun close(cut: Boolean) {
        val s = said ?: return
        said = null
        val cb = transcriptCb ?: return
        val full = java.text.Normalizer.normalize(turns.getOrNull(s.turn)?.line ?: "", java.text.Normalizer.Form.NFC)
        val truncated = cut && s.role == TranscriptRole.ASSISTANT
        cb(TranscriptUpdate(s.role, (if (truncated) s.text else full).trim(), true, s.id, truncated))
    }

    companion object {
        /** The built-in samples, from the engine: `calendar`, `quick-answer`, `long-answer`, `barge-in`. */
        val sampleNames: List<String> get() = conversationSampleNames()

        /**
         * The same names without calling the engine -- for UI built before the native library
         * loads (a JVM unit test has none). `SimulatedVoiceSourceTests` holds it equal to [sampleNames].
         */
        val BUILT_IN_SAMPLES: List<String> = listOf("calendar", "quick-answer", "long-answer", "barge-in")

        /** A built-in sample by name. */
        fun sample(name: String, bands: Int = 16, loop: Boolean? = null): SimulatedVoiceSource {
            val json =
                conversationSample(name)
                    ?: throw IllegalArgumentException("SimulatedVoiceSource: no sample named \"$name\"")
            return SimulatedVoiceSource(json, bands, loop = loop)
        }
    }
}
