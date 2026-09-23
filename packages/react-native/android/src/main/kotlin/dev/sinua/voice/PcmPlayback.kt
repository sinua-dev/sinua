// The native `PcmAudioGraph` (packages/voice/src/PcmAudioGraph.ts), mirrored
// from packages/ios SinuaVoice/PcmPlayback.swift. See that file for the
// design notes; the Kotlin is a line-for-line mirror.
package dev.sinua.voice

/** `queued` (scheduled, not yet audible), `audible`, `drained` -- Web `PcmAudioGraph`'s `PlaybackState`. */
enum class PlaybackState { QUEUED, AUDIBLE, DRAINED }

/**
 * The playback timeline in frames: a new burst starts `leadFrames` after "now"
 * (Google's `initialBufferTime`, 0.1 s), later chunks follow gap-free at the
 * cursor; recent samples are kept with their frame positions so the analyser
 * is fed exactly what has *played*. Pure: the player only supplies the clock.
 */
class PlaybackTimeline(val rate: Int, leadSeconds: Double = 0.1) {
    val leadFrames: Long = Math.round(leadSeconds * rate)
    var cursor = 0L
        private set
    var burstStart = 0L
        private set
    private class Chunk(val start: Long, val samples: FloatArray)
    private val chunks = ArrayDeque<Chunk>()
    private val historyFrames = rate.toLong() // 1 s

    /** Schedule `samples` given the current played frame; returns the frame it starts at. */
    fun append(samples: FloatArray, now: Long): Long {
        if (samples.isEmpty()) return cursor
        if (cursor <= now) {
            cursor = now + leadFrames
            burstStart = cursor
        }
        val start = cursor
        chunks.addLast(Chunk(start, samples))
        cursor += samples.size
        prune(now - historyFrames)
        return start
    }

    fun state(now: Long): PlaybackState = when {
        cursor == 0L || now >= cursor -> PlaybackState.DRAINED
        now >= burstStart -> PlaybackState.AUDIBLE
        else -> PlaybackState.QUEUED
    }

    /** Samples covering frames `[from, to)`, silence where nothing was scheduled; only the newest `maxCount`. */
    fun played(from: Long, to: Long, maxCount: Int = 4096): FloatArray {
        if (to <= from) return FloatArray(0)
        val lo = maxOf(from, to - maxCount)
        val out = FloatArray((to - lo).toInt())
        for (c in chunks) {
            val a = maxOf(lo, c.start)
            val b = minOf(to, c.start + c.samples.size)
            for (f in a until b) out[(f - lo).toInt()] = c.samples[(f - c.start).toInt()]
        }
        prune(to - historyFrames)
        return out
    }

    /** Interrupt / reconnect: drop everything; the player's clock restarts at 0 too. */
    fun clear() {
        chunks.clear()
        cursor = 0
        burstStart = 0
    }

    private fun prune(before: Long) {
        while (chunks.isNotEmpty() && chunks.first().let { it.start + it.samples.size <= before }) chunks.removeFirst()
    }
}

/**
 * What a platform audio stack provides to `PcmAudioGraph`: capture at a fixed
 * input rate, and a player at a fixed output rate that can schedule a buffer at
 * a frame of its own clock. `AndroidPcmAudioDevice` is the real one; tests use a
 * fake whose clock they drive.
 */
interface PcmAudioDevice {
    /** Starts capture + playback. `onCapture` runs on a capture thread with mono floats at `inputRate`. */
    fun start(inputRate: Int, outputRate: Int, onCapture: (FloatArray) -> Unit)

    /** Frames played since the last start / `resetPlayback()`, on the player's clock. */
    val playedFrames: Long

    /** Play `samples` starting at `frame` on the player's clock (never earlier than now). */
    fun schedule(samples: FloatArray, frame: Long)

    /** Stop and drop everything scheduled; the clock restarts at 0. */
    fun resetPlayback(fade: Boolean)
    fun stop()

    /** Set by `PcmAudioGraph`: the device's clock restarted and its scheduled audio is gone. Main thread. */
    var onClockReset: (() -> Unit)?
}

/**
 * Mic capture at the vendor's input rate, gap-free scheduled playback at its
 * output rate, and `AudioAnalysis` fed from the *played* samples. Shared by the
 * PCM-over-socket vendors. Main thread, except `onCapture`.
 */
class PcmAudioGraph(val device: PcmAudioDevice) {
    var timeline = PlaybackTimeline(24000)
        private set
    private val spectrum = SpectrumAnalyser()
    private val analysis = AudioAnalysis()
    private var analysedTo = 0L
    private var running = false

    init {
        device.onClockReset = {
            timeline.clear()
            analysedTo = 0
        }
    }

    fun start(inputRate: Int, outputRate: Int, onCapture: (FloatArray) -> Unit) {
        timeline = PlaybackTimeline(outputRate)
        spectrum.reset()
        analysis.reset()
        analysedTo = 0
        device.start(inputRate, outputRate, onCapture)
        running = true
    }

    /** Schedule decoded samples; a `rate` other than the output rate is resampled. */
    fun enqueue(samples: FloatArray, rate: Int) {
        if (!running || samples.isEmpty()) return
        val s = Pcm.resample(samples, rate, timeline.rate)
        val at = timeline.append(s, device.playedFrames)
        device.schedule(s, at)
    }

    fun playbackState(): PlaybackState = if (running) timeline.state(device.playedFrames) else PlaybackState.DRAINED

    /** Metrics of what has played since the last read, or null before `start`. */
    fun read(): VoiceMetrics? {
        if (!running) return null
        val now = device.playedFrames
        if (now < analysedTo) analysedTo = now // clock restarted
        spectrum.push(timeline.played(analysedTo, now, spectrum.fftSize))
        analysedTo = now
        return analysis.read(spectrum.byteFrequencyData())
    }

    fun clearPlayback(fade: Boolean) {
        timeline.clear()
        if (!running) return
        device.resetPlayback(fade)
        analysedTo = 0
    }

    fun stop() {
        if (running) device.stop()
        running = false
        timeline.clear()
        analysedTo = 0
    }
}
