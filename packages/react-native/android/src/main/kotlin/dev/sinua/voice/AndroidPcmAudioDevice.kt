package dev.sinua.voice

import android.annotation.SuppressLint
import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioRecord
import android.media.AudioTrack
import android.media.MediaRecorder
import android.media.audiofx.AcousticEchoCanceler
import java.util.concurrent.LinkedBlockingQueue
import java.util.concurrent.TimeUnit

/**
 * `PcmAudioDevice` on `AudioRecord` + a streaming `AudioTrack` (the iOS twin is
 * `AVPcmAudioDevice`).
 *
 * - Capture: `VOICE_COMMUNICATION` source at the vendor's input rate, mono
 *   float, so the platform's echo canceller applies (plus `AcousticEchoCanceler`
 *   when the device offers one). Without it the model hears itself on speaker
 *   and barges in on its own voice; Web gets this from `getUserMedia`'s
 *   default `echoCancellation`.
 * - Playback: a `MODE_STREAM` float track at the output rate, fed by one writer
 *   thread. A buffer scheduled at frame F is preceded by silence up to F, so
 *   the track's `playbackHeadPosition` is exactly the timeline's frame clock.
 *   While the track has nothing to play, its head stalls, and a new burst
 *   (lead of silence first) starts from where it stopped.
 *
 * The app must hold RECORD_AUDIO (`isMicPermissionGranted`). Not exercised on a
 * device or emulator yet: tests don't open real audio on the shared machine
 * (docs/audio-pipeline.md).
 */
class AndroidPcmAudioDevice(private val voiceCommunication: Boolean = true) : PcmAudioDevice {
    override var onClockReset: (() -> Unit)? = null

    private var record: AudioRecord? = null
    private var track: AudioTrack? = null
    private var aec: AcousticEchoCanceler? = null
    private var reader: Thread? = null
    private var writer: Thread? = null

    @Volatile private var running = false

    private class Item(val generation: Int, val frame: Long, val samples: FloatArray)
    private val queue = LinkedBlockingQueue<Item>()

    /** Bumped by `resetPlayback`: the writer drops items and restarts its frame count. */
    @Volatile private var generation = 0

    @SuppressLint("MissingPermission") // the caller checks RECORD_AUDIO, as with LocalMicVoiceSource
    override fun start(inputRate: Int, outputRate: Int, onCapture: (FloatArray) -> Unit) {
        val inMin = AudioRecord.getMinBufferSize(inputRate, AudioFormat.CHANNEL_IN_MONO, AudioFormat.ENCODING_PCM_FLOAT)
        val rec = AudioRecord(
            if (voiceCommunication) MediaRecorder.AudioSource.VOICE_COMMUNICATION else MediaRecorder.AudioSource.MIC,
            inputRate,
            AudioFormat.CHANNEL_IN_MONO,
            AudioFormat.ENCODING_PCM_FLOAT,
            maxOf(inMin, CAPTURE_CHUNK * 4 * 2),
        )
        check(rec.state == AudioRecord.STATE_INITIALIZED) { "AudioRecord failed to initialize" }
        if (voiceCommunication && AcousticEchoCanceler.isAvailable()) {
            aec = AcousticEchoCanceler.create(rec.audioSessionId)?.apply { enabled = true }
        }
        val outMin = AudioTrack.getMinBufferSize(
            outputRate,
            AudioFormat.CHANNEL_OUT_MONO,
            AudioFormat.ENCODING_PCM_FLOAT,
        )
        val trk = AudioTrack.Builder()
            .setAudioAttributes(
                AudioAttributes.Builder()
                    .setUsage(
                        if (voiceCommunication) {
                            AudioAttributes.USAGE_VOICE_COMMUNICATION
                        } else {
                            AudioAttributes.USAGE_MEDIA
                        },
                    )
                    .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                    .build(),
            )
            .setAudioFormat(
                AudioFormat.Builder()
                    .setEncoding(AudioFormat.ENCODING_PCM_FLOAT)
                    .setSampleRate(outputRate)
                    .setChannelMask(AudioFormat.CHANNEL_OUT_MONO)
                    .build(),
            )
            .setTransferMode(AudioTrack.MODE_STREAM)
            .setBufferSizeInBytes(outMin * 2)
            .build()
        record = rec
        track = trk
        running = true
        rec.startRecording()
        trk.play()

        reader = Thread({
            val buf = FloatArray(CAPTURE_CHUNK)
            while (running) {
                val n = rec.read(buf, 0, buf.size, AudioRecord.READ_BLOCKING)
                if (n > 0) onCapture(buf.copyOf(n))
            }
        }, "sinua-pcm-capture").apply { start() }

        writer = Thread({
            var gen = generation
            var written = 0L
            val silence = FloatArray(1024)
            while (running) {
                val item = queue.poll(50, TimeUnit.MILLISECONDS) ?: continue
                if (item.generation != generation) continue
                if (gen != item.generation) {
                    gen = item.generation
                    written = 0
                }
                var pad = item.frame - written
                while (pad > 0 && running && gen == generation) {
                    val k = minOf(pad, silence.size.toLong()).toInt()
                    trk.write(silence, 0, k, AudioTrack.WRITE_BLOCKING)
                    written += k
                    pad -= k
                }
                var off = 0
                while (off < item.samples.size && running && gen == generation) {
                    val k = minOf(1024, item.samples.size - off)
                    trk.write(item.samples, off, k, AudioTrack.WRITE_BLOCKING)
                    written += k
                    off += k
                }
            }
        }, "sinua-pcm-playback").apply { start() }
    }

    /** `playbackHeadPosition` is an unsigned 32-bit frame count (docs); reset to 0 by `flush()`. */
    override val playedFrames: Long
        get() = (track?.playbackHeadPosition ?: 0).toLong() and 0xFFFF_FFFFL

    override fun schedule(samples: FloatArray, frame: Long) {
        queue.put(Item(generation, frame, samples))
    }

    override fun resetPlayback(fade: Boolean) {
        // `fade` isn't ported: the Web 30 ms ramp would need per-sample gain on
        // already-written data. Whether the hard cut clicks is a device check.
        generation++
        queue.clear()
        track?.run {
            pause()
            flush() // drops unplayed data and resets the head position to 0
            play()
        }
    }

    override fun stop() {
        running = false
        generation++
        queue.clear()
        // Stop first: that unblocks a READ_BLOCKING read / WRITE_BLOCKING write, then join.
        record?.stop()
        track?.run {
            pause()
            flush()
        }
        reader?.join(500)
        writer?.join(500)
        reader = null
        writer = null
        record?.release()
        record = null
        aec?.release()
        aec = null
        track?.release()
        track = null
    }

    companion object {
        /** Samples per mic chunk at the input rate (~64 ms at 16 kHz), as on Web. */
        const val CAPTURE_CHUNK = 1024
    }
}
