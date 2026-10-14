package dev.sinua.voice

import android.content.Context
import android.media.AudioAttributes
import android.media.AudioDeviceCallback
import android.media.AudioDeviceInfo
import android.media.AudioFocusRequest
import android.media.AudioManager
import android.os.Build
import android.os.Handler
import android.os.Looper
import android.util.Log

/**
 * How a voice source sets up the app's audio for a call (the iOS `VoiceAudioSession`'s
 * twin). Without it the call plays as a voice call in the normal mode: through the
 * earpiece or at call volume (very quiet), and some devices record silence.
 */
enum class VoiceAudioSession {
    /**
     * Communication mode, loudspeaker by default: a Bluetooth or wired headset still wins.
     * The default, as LiveKit's own audio handling.
     */
    SPEAKER,

    /** Communication mode through the earpiece, a phone call's routing (headsets still win). */
    RECEIVER,

    /** The app sets up the audio itself; the source never touches `AudioManager`. */
    UNMANAGED,
}

/** The outputs the policy chooses between. */
enum class AudioRoute { BLUETOOTH, WIRED, SPEAKER, EARPIECE }

object AudioRoutePolicy {
    /**
     * The communication device for [session] among [available] outputs, or null to leave
     * the routing alone. Headsets beat the loudspeaker, so a forced speaker never overrides
     * them (the iOS rule against `overrideOutputAudioPort`).
     */
    fun choose(session: VoiceAudioSession, available: Set<AudioRoute>): AudioRoute? = when {
        session == VoiceAudioSession.UNMANAGED -> null
        AudioRoute.BLUETOOTH in available -> AudioRoute.BLUETOOTH
        AudioRoute.WIRED in available -> AudioRoute.WIRED
        session == VoiceAudioSession.SPEAKER && AudioRoute.SPEAKER in available -> AudioRoute.SPEAKER
        AudioRoute.EARPIECE in available -> AudioRoute.EARPIECE
        AudioRoute.SPEAKER in available -> AudioRoute.SPEAKER
        else -> null
    }
}

/** What [AudioSessionClaims] needs from the platform; [AndroidAudioSessionPort] in apps, a fake in tests. */
interface AudioSessionPort {
    /** `AudioManager.mode`. */
    var mode: Int
    fun availableRoutes(): Set<AudioRoute>
    fun route(to: AudioRoute)

    /** Undoes [route]: the platform picks the device again. */
    fun clearRoute()
    fun requestFocus()
    fun abandonFocus()

    /** Calls [onChange] (main thread) when an output appears or goes, until [unwatchDevices]. */
    fun watchDevices(onChange: () -> Unit)
    fun unwatchDevices()
}

/**
 * Who holds the app's audio among Sinua's voice sources: the first claim saves the mode,
 * enters communication mode, takes audio focus and routes; the last release puts it all
 * back. One source's disconnect never cuts another's audio, and a source that never
 * claimed ([VoiceAudioSession.UNMANAGED]) never touches anything.
 */
class AudioSessionClaims {
    private class Active(val port: AudioSessionPort, val session: VoiceAudioSession, val savedMode: Int)

    private var holders = 0
    private var active: Active? = null

    /** Before a source's audio starts. False (nothing claimed) for [VoiceAudioSession.UNMANAGED]. */
    @Synchronized
    fun claim(session: VoiceAudioSession, port: () -> AudioSessionPort): Boolean {
        if (session == VoiceAudioSession.UNMANAGED) return false
        if (holders == 0) {
            val p = port()
            val a = Active(p, session, p.mode)
            active = a
            p.mode = AudioManager.MODE_IN_COMMUNICATION
            p.requestFocus()
            routeNow(a)
            p.watchDevices { reroute() }
        }
        holders += 1
        return true
    }

    /** When a source that claimed is done; the last release restores the app's audio. */
    @Synchronized
    fun release() {
        if (holders == 0) return
        holders -= 1
        if (holders > 0) return
        val a = active ?: return
        active = null
        a.port.unwatchDevices()
        a.port.clearRoute()
        a.port.abandonFocus()
        a.port.mode = a.savedMode
    }

    /** A headset came or went: choose again. */
    @Synchronized
    fun reroute() {
        active?.let(::routeNow)
    }

    private fun routeNow(a: Active) {
        AudioRoutePolicy.choose(a.session, a.port.availableRoutes())?.let(a.port::route)
    }

    companion object {
        /** The one every source in the process shares. */
        val shared = AudioSessionClaims()

        /**
         * Claims for a source through [AudioSessionClaims.shared] with the real `AudioManager`.
         * Never throws: a device that refuses a step keeps the call going without it.
         */
        fun claim(context: Context, session: VoiceAudioSession): Boolean = try {
            shared.claim(session) { AndroidAudioSessionPort(context) }
        } catch (e: RuntimeException) {
            log("couldn't set up the call's audio: ${e.message}")
            false
        }

        fun release() {
            try {
                shared.release()
            } catch (e: RuntimeException) {
                log("couldn't restore the app's audio: ${e.message}")
            }
        }

        internal fun log(msg: String) {
            try {
                Log.w("SinuaAudio", msg)
            } catch (_: RuntimeException) {
                // android.util.Log is a stub under plain JVM tests.
            }
        }
    }
}

/**
 * [AudioSessionPort] on `AudioManager` (MODIFY_AUDIO_SETTINGS, which the voice modules
 * declare). API 31+: `setCommunicationDevice`; below: speakerphone and Bluetooth SCO.
 */
class AndroidAudioSessionPort(context: Context) : AudioSessionPort {
    private val am = context.applicationContext.getSystemService(Context.AUDIO_SERVICE) as AudioManager
    private val main = Handler(Looper.getMainLooper())
    private var focus: AudioFocusRequest? = null
    private var callback: AudioDeviceCallback? = null
    private var savedSpeakerphone: Boolean? = null
    private var sco = false

    override var mode: Int
        get() = am.mode
        set(value) {
            am.mode = value
        }

    override fun availableRoutes(): Set<AudioRoute> {
        val devices = if (Build.VERSION.SDK_INT >= 31) {
            am.availableCommunicationDevices
        } else {
            am.getDevices(AudioManager.GET_DEVICES_OUTPUTS).toList()
        }
        return devices.mapNotNull { routeOf(it.type) }.toSet()
    }

    override fun route(to: AudioRoute) {
        if (Build.VERSION.SDK_INT >= 31) {
            val device = am.availableCommunicationDevices.firstOrNull { routeOf(it.type) == to } ?: return
            if (!am.setCommunicationDevice(device)) AudioSessionClaims.log("setCommunicationDevice($to) refused")
            return
        }
        @Suppress("DEPRECATION")
        run {
            if (savedSpeakerphone == null) savedSpeakerphone = am.isSpeakerphoneOn
            if (to == AudioRoute.BLUETOOTH) {
                am.startBluetoothSco()
                am.isBluetoothScoOn = true
                sco = true
            } else {
                stopSco()
            }
            am.isSpeakerphoneOn = to == AudioRoute.SPEAKER
        }
    }

    override fun clearRoute() {
        if (Build.VERSION.SDK_INT >= 31) {
            am.clearCommunicationDevice()
            return
        }
        stopSco()
        @Suppress("DEPRECATION")
        savedSpeakerphone?.let { am.isSpeakerphoneOn = it }
        savedSpeakerphone = null
    }

    @Suppress("DEPRECATION")
    private fun stopSco() {
        if (!sco) return
        sco = false
        am.isBluetoothScoOn = false
        am.stopBluetoothSco()
    }

    override fun requestFocus() {
        if (Build.VERSION.SDK_INT >= 26) {
            val r = AudioFocusRequest.Builder(AudioManager.AUDIOFOCUS_GAIN)
                .setAudioAttributes(
                    AudioAttributes.Builder()
                        .setUsage(AudioAttributes.USAGE_VOICE_COMMUNICATION)
                        .setContentType(AudioAttributes.CONTENT_TYPE_SPEECH)
                        .build(),
                )
                .setOnAudioFocusChangeListener({}, main)
                .build()
            focus = r
            am.requestAudioFocus(r)
        } else {
            @Suppress("DEPRECATION")
            am.requestAudioFocus(null, AudioManager.STREAM_VOICE_CALL, AudioManager.AUDIOFOCUS_GAIN)
        }
    }

    override fun abandonFocus() {
        if (Build.VERSION.SDK_INT >= 26) {
            focus?.let { am.abandonAudioFocusRequest(it) }
            focus = null
        } else {
            @Suppress("DEPRECATION")
            am.abandonAudioFocus(null)
        }
    }

    override fun watchDevices(onChange: () -> Unit) {
        val cb = object : AudioDeviceCallback() {
            override fun onAudioDevicesAdded(added: Array<out AudioDeviceInfo>) = onChange()
            override fun onAudioDevicesRemoved(removed: Array<out AudioDeviceInfo>) = onChange()
        }
        callback = cb
        am.registerAudioDeviceCallback(cb, main)
    }

    override fun unwatchDevices() {
        callback?.let { am.unregisterAudioDeviceCallback(it) }
        callback = null
    }

    private fun routeOf(type: Int): AudioRoute? = when (type) {
        AudioDeviceInfo.TYPE_BLUETOOTH_SCO, AudioDeviceInfo.TYPE_BLE_HEADSET -> AudioRoute.BLUETOOTH

        AudioDeviceInfo.TYPE_WIRED_HEADSET, AudioDeviceInfo.TYPE_WIRED_HEADPHONES, AudioDeviceInfo.TYPE_USB_HEADSET ->
            AudioRoute.WIRED

        AudioDeviceInfo.TYPE_BUILTIN_SPEAKER -> AudioRoute.SPEAKER

        AudioDeviceInfo.TYPE_BUILTIN_EARPIECE -> AudioRoute.EARPIECE

        else -> null
    }
}
