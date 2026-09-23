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
            spectrum.push(generator.next((sampleRate / UPDATE_HZ).toInt()))
            val m = analysis.read(spectrum.byteFrequencyData())
            metricsCb?.invoke(m)
            stateCb?.invoke(if (m.level > SPEAKING_LEVEL) AgentState.SPEAKING else AgentState.LISTENING)
            handler.postDelayed(this, (1000 / UPDATE_HZ).toLong())
        }
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
        handler.post(tick)
    }

    override fun disconnect() {
        running = false
        handler.removeCallbacks(tick)
        stateCb?.invoke(AgentState.IDLE)
    }

    companion object {
        const val UPDATE_HZ = 30.0
    }
}
