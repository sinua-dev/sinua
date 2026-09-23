package dev.sinua.voice

import android.annotation.SuppressLint
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.MediaRecorder
import android.os.Handler
import android.os.Looper

/**
 * The device microphone as a [VoiceSource]: `AudioRecord` (mono float PCM,
 * 48 kHz with a 44.1 kHz fallback -- the only rate Android guarantees)
 * feeding the same spec-exact [SpectrumAnalyser] -> [AudioAnalysis] path the
 * Web Studio's `LocalMicVoiceSource` uses (docs/audio-pipeline.md,
 * *Native*). A reader thread fills a ring; a 30 Hz main-looper tick
 * analyses the newest samples. `speaking` when level > 0.08, else
 * `listening` -- Web's heuristic.
 *
 * The *app* declares `android.permission.RECORD_AUDIO` and requests it at
 * runtime before [connect] (this library's manifest deliberately doesn't,
 * so it never adds a mic permission to an app that doesn't use it);
 * without it [connect] throws `SecurityException`.
 */
class LocalMicVoiceSource : VoiceSource {
    private val handler = Handler(Looper.getMainLooper())
    private val ring = SampleRing(4096)
    private val spectrum = SpectrumAnalyser()
    private val analysis = AudioAnalysis()
    private var record: AudioRecord? = null
    private var reader: Thread? = null

    @Volatile private var running = false
    private var metricsCb: ((VoiceMetrics) -> Unit)? = null
    private var stateCb: ((AgentState) -> Unit)? = null

    private val tick = object : Runnable {
        override fun run() {
            if (!running) return
            val samples = ring.drain()
            spectrum.push(samples)
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

    /** Throws `SecurityException` without RECORD_AUDIO, `IllegalStateException` if no mic can be opened. */
    @SuppressLint("MissingPermission") // documented: the app requests RECORD_AUDIO first
    override fun connect() {
        stateCb?.invoke(AgentState.INITIALIZING)
        try {
            val rec = openRecord()
            record = rec
            rec.startRecording()
            running = true
            spectrum.reset()
            analysis.reset()
            val chunk = FloatArray(1024)
            reader = Thread({
                while (running) {
                    val n = rec.read(chunk, 0, chunk.size, AudioRecord.READ_BLOCKING)
                    if (n > 0) ring.write(chunk, n)
                }
            }, "sinua-mic").also { it.start() }
            stateCb?.invoke(AgentState.LISTENING)
            handler.post(tick)
        } catch (e: Exception) {
            stop()
            stateCb?.invoke(AgentState.IDLE)
            throw e
        }
    }

    override fun disconnect() {
        stop()
        stateCb?.invoke(AgentState.IDLE)
    }

    private fun stop() {
        running = false
        handler.removeCallbacks(tick)
        record?.let {
            try {
                it.stop()
            } catch (_: IllegalStateException) {
                // never started
            }
            it.release()
        }
        record = null
        reader?.join(200)
        reader = null
    }

    @SuppressLint("MissingPermission")
    private fun openRecord(): AudioRecord {
        for (rate in intArrayOf(48_000, 44_100)) {
            val min = AudioRecord.getMinBufferSize(rate, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_FLOAT)
            if (min <= 0) continue
            val rec =
                AudioRecord(
                    MediaRecorder.AudioSource.MIC,
                    rate,
                    AudioFormat.CHANNEL_IN_MONO,
                    AudioFormat.ENCODING_PCM_FLOAT,
                    maxOf(
                        min,
                        4096 * 4,
                    ),
                )
            if (rec.state == AudioRecord.STATE_INITIALIZED) return rec
            rec.release()
        }
        throw IllegalStateException("No microphone could be opened at 48 or 44.1 kHz")
    }

    companion object {
        const val UPDATE_HZ = 30.0
    }
}

/** Reader-thread -> main-thread sample hand-off; keeps only the newest `capacity` samples. */
internal class SampleRing(capacity: Int) {
    private val buf = FloatArray(capacity)
    private var count = 0
    private var head = 0

    @Synchronized
    fun write(samples: FloatArray, n: Int = samples.size) {
        for (i in 0 until n) {
            buf[head] = samples[i]
            head = (head + 1) % buf.size
            count = minOf(count + 1, buf.size)
        }
    }

    /** Everything written since the last drain, oldest first. */
    @Synchronized
    fun drain(): FloatArray {
        val start = (head - count + buf.size) % buf.size
        val out = FloatArray(count) { buf[(start + it) % buf.size] }
        count = 0
        return out
    }
}

/** Whether the app currently holds RECORD_AUDIO (request it with the platform's permission flow first). */
fun isMicPermissionGranted(context: android.content.Context): Boolean =
    context.checkSelfPermission(android.Manifest.permission.RECORD_AUDIO) ==
        android.content.pm.PackageManager.PERMISSION_GRANTED
