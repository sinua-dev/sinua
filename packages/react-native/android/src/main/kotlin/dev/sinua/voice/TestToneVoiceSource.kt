package dev.sinua.voice

import android.os.Handler
import android.os.Looper

/**
 * A [VoiceSource] needing no microphone and no vendor: Web's test tone
 * ([TestToneGenerator]), synthesized in software and run through the exact
 * same [SpectrumAnalyser] -> [AudioAnalysis] path as the mic. Nothing is
 * played. 30 Hz ticks on the main looper.
 */
class TestToneVoiceSource(private val sampleRate: Double = 48_000.0) : VoiceSource {
    private val handler = Handler(Looper.getMainLooper())
    private var generator = TestToneGenerator(sampleRate)
    private val spectrum = SpectrumAnalyser()
    private val analysis = AudioAnalysis()
    private var running = false
    private var metricsCb: ((VoiceMetrics) -> Unit)? = null
    private var stateCb: ((AgentState) -> Unit)? = null

    private val tick = object : Runnable {
        override fun run() {
            if (!running) return
            val n = (sampleRate / UPDATE_HZ).toInt()
            val samples = generator.next(n)
            spectrum.push(if (muted) FloatArray(n) else samples)
            val m = analysis.read(spectrum.byteFrequencyData())
            metricsCb?.invoke(m)
            stateCb?.invoke(if (m.level > SPEAKING_LEVEL) AgentState.SPEAKING else AgentState.LISTENING)
            handler.postDelayed(this, (1000 / UPDATE_HZ).toLong())
        }
    }

    override val supportsMute: Boolean get() = true
    override val reportsConnection: Boolean get() = true
    private var connectionCb: ((Boolean) -> Unit)? = null
    private var muted = false

    override fun onConnectionChange(cb: (Boolean) -> Unit) {
        connectionCb = cb
    }

    /** Muted, the tone stands in for a muted mic: the level reads 0. */
    override fun setMuted(muted: Boolean) {
        this.muted = muted
    }

    override fun onMetrics(cb: (VoiceMetrics) -> Unit) {
        metricsCb = cb
    }

    override fun onStateChange(cb: (AgentState) -> Unit) {
        stateCb = cb
    }

    override fun connect() {
        stateCb?.invoke(AgentState.INITIALIZING)
        generator = TestToneGenerator(sampleRate)
        spectrum.reset()
        analysis.reset()
        running = true
        stateCb?.invoke(AgentState.LISTENING)
        connectionCb?.invoke(true)
        handler.post(tick)
    }

    override fun disconnect() {
        val was = running
        running = false
        handler.removeCallbacks(tick)
        stateCb?.invoke(AgentState.IDLE)
        if (was) connectionCb?.invoke(false)
    }

    companion object {
        const val UPDATE_HZ = 30.0
    }
}
