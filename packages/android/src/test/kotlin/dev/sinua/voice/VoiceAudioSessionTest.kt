package dev.sinua.voice

import android.media.AudioManager
import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Test

/**
 * The Android audio-session policy over a fake `AudioManager` (the iOS
 * VoiceAudioSessionTests' twin): which output a call takes, and that the claims save and
 * restore the app's audio around it.
 */
class VoiceAudioSessionTest {
    private class FakePort(var outputs: Set<AudioRoute>) : AudioSessionPort {
        override var mode = AudioManager.MODE_NORMAL
        var routed: AudioRoute? = null
        var focused = false
        var watcher: (() -> Unit)? = null
        val calls = mutableListOf<String>()

        override fun availableRoutes() = outputs
        override fun route(to: AudioRoute) {
            routed = to
            calls += "route $to"
        }
        override fun clearRoute() {
            routed = null
            calls += "clear"
        }
        override fun requestFocus() {
            focused = true
            calls += "focus"
        }
        override fun abandonFocus() {
            focused = false
            calls += "abandon"
        }
        override fun watchDevices(onChange: () -> Unit) {
            watcher = onChange
        }
        override fun unwatchDevices() {
            watcher = null
        }
    }

    private val phone = setOf(AudioRoute.SPEAKER, AudioRoute.EARPIECE)

    @Test
    fun speakerTakesTheLoudspeakerOnAPhone() {
        assertEquals(AudioRoute.SPEAKER, AudioRoutePolicy.choose(VoiceAudioSession.SPEAKER, phone))
    }

    @Test
    fun receiverTakesTheEarpiece() {
        assertEquals(AudioRoute.EARPIECE, AudioRoutePolicy.choose(VoiceAudioSession.RECEIVER, phone))
    }

    @Test
    fun headsetsBeatTheLoudspeaker() {
        assertEquals(AudioRoute.WIRED, AudioRoutePolicy.choose(VoiceAudioSession.SPEAKER, phone + AudioRoute.WIRED))
        assertEquals(
            AudioRoute.BLUETOOTH,
            AudioRoutePolicy.choose(VoiceAudioSession.SPEAKER, phone + AudioRoute.WIRED + AudioRoute.BLUETOOTH),
        )
        assertEquals(AudioRoute.BLUETOOTH, AudioRoutePolicy.choose(VoiceAudioSession.RECEIVER, phone + AudioRoute.BLUETOOTH))
    }

    @Test
    fun aDeviceWithoutTheWantedOutputFallsBack() {
        // A tablet has no earpiece; a speakerless device has no loudspeaker.
        assertEquals(AudioRoute.SPEAKER, AudioRoutePolicy.choose(VoiceAudioSession.RECEIVER, setOf(AudioRoute.SPEAKER)))
        assertEquals(AudioRoute.EARPIECE, AudioRoutePolicy.choose(VoiceAudioSession.SPEAKER, setOf(AudioRoute.EARPIECE)))
        assertNull(AudioRoutePolicy.choose(VoiceAudioSession.SPEAKER, emptySet()))
    }

    @Test
    fun unmanagedChoosesNothing() {
        assertNull(AudioRoutePolicy.choose(VoiceAudioSession.UNMANAGED, phone + AudioRoute.WIRED))
    }

    @Test
    fun aClaimEntersCommunicationModeTakesFocusAndRoutes() {
        val port = FakePort(phone)
        port.mode = AudioManager.MODE_NORMAL
        assertTrue(AudioSessionClaims().claim(VoiceAudioSession.SPEAKER) { port })
        assertEquals(AudioManager.MODE_IN_COMMUNICATION, port.mode)
        assertTrue(port.focused)
        assertEquals(AudioRoute.SPEAKER, port.routed)
        assertTrue(port.watcher != null)
    }

    @Test
    fun theReleaseRestoresTheAppsAudio() {
        val port = FakePort(phone)
        port.mode = AudioManager.MODE_RINGTONE // whatever the app had
        val claims = AudioSessionClaims()
        claims.claim(VoiceAudioSession.SPEAKER) { port }
        claims.release()
        assertEquals(AudioManager.MODE_RINGTONE, port.mode)
        assertFalse(port.focused)
        assertNull(port.routed)
        assertNull(port.watcher)
    }

    @Test
    fun twoSourcesShareOneClaim() {
        val port = FakePort(phone)
        var made = 0
        val claims = AudioSessionClaims()
        claims.claim(VoiceAudioSession.SPEAKER) {
            made++
            port
        }
        claims.claim(VoiceAudioSession.SPEAKER) {
            made++
            port
        }
        assertEquals(1, made)
        claims.release() // one source disconnects: the other's audio stays
        assertEquals(AudioManager.MODE_IN_COMMUNICATION, port.mode)
        assertEquals(AudioRoute.SPEAKER, port.routed)
        claims.release()
        assertEquals(AudioManager.MODE_NORMAL, port.mode)
        claims.release() // an extra release does nothing
        assertEquals(listOf("focus", "route SPEAKER", "clear", "abandon"), port.calls)
    }

    @Test
    fun unmanagedTouchesNothing() {
        val claims = AudioSessionClaims()
        assertFalse(claims.claim(VoiceAudioSession.UNMANAGED) { error("no AudioManager for unmanaged") })
        // Nothing was claimed, so a later managed source is the first holder.
        val port = FakePort(phone)
        claims.claim(VoiceAudioSession.SPEAKER) { port }
        claims.release()
        assertEquals(AudioManager.MODE_NORMAL, port.mode)
    }

    @Test
    fun aHeadsetPluggedInMidCallTakesOverAndUnpluggingGoesBack() {
        val port = FakePort(phone)
        AudioSessionClaims().claim(VoiceAudioSession.SPEAKER) { port }
        port.outputs = phone + AudioRoute.WIRED
        port.watcher!!()
        assertEquals(AudioRoute.WIRED, port.routed)
        port.outputs = phone
        port.watcher!!()
        assertEquals(AudioRoute.SPEAKER, port.routed)
    }
}
