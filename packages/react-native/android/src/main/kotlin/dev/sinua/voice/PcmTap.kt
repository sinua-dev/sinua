package dev.sinua.voice

/**
 * Analyses PCM tapped from a playing stream (e.g. a remote WebRTC track's sink):
 * the audio thread writes into `sink`, the main thread's `read()` drains it
 * through the same `SpectrumAnalyser` -> `AudioAnalysis` path as the mic. Mirrors
 * packages/ios SinuaVoice/PcmTap.swift.
 */
class PcmTap {
    val sink = LiveKitPcmSink()
    private val spectrum = SpectrumAnalyser()
    private val analysis = AudioAnalysis()

    fun reset() {
        sink.ring.drain()
        spectrum.reset()
        analysis.reset()
    }

    /** Main thread, ~30 Hz. */
    fun read(): VoiceMetrics {
        spectrum.push(sink.ring.drain())
        return analysis.read(spectrum.byteFrequencyData())
    }
}
