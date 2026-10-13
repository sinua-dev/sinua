package dev.sinua.minifysmoke

import android.os.Bundle
import android.util.Log
import androidx.activity.ComponentActivity
import androidx.activity.compose.setContent
import androidx.compose.foundation.layout.size
import androidx.compose.ui.Modifier
import androidx.compose.ui.unit.dp
import dev.sinua.view.SinuaView
import livekit.org.webrtc.DataChannel
import livekit.org.webrtc.IceCandidate
import livekit.org.webrtc.MediaStream
import livekit.org.webrtc.PeerConnection
import livekit.org.webrtc.PeerConnectionFactory
import uniffi.core_engine.resolveFxSpec

/**
 * Logs `SinuaSmoke: engine ok=…` after a direct engine call, `SinuaSmoke: webrtc ok` once
 * WebRTC (what sinua-openai's connect() starts with) has loaded its native library and built a
 * factory and a peer connection, then `SinuaSmoke: frame` once the view has drawn its first
 * frame. scripts/android-minify-smoke.sh waits for all three; a crash (WebRTC aborts natively
 * when R8 has stripped a class its JNI_OnLoad looks up by name), an UnsatisfiedLinkError or a
 * missing line fails it. No track is created: no microphone, no playback.
 */
class MainActivity : ComponentActivity() {
    private var reported = false

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        val ok = resolveFxSpec("""{"fxSpec":"1.8","object":"orb","pattern":"glowing"}""").ok
        Log.i(TAG, "engine ok=$ok")
        PeerConnectionFactory.initialize(
            PeerConnectionFactory.InitializationOptions.builder(applicationContext).createInitializationOptions(),
        )
        val factory = PeerConnectionFactory.builder().createPeerConnectionFactory()
        val pc = factory.createPeerConnection(PeerConnection.RTCConfiguration(emptyList()), NoOpObserver())
        Log.i(TAG, "webrtc ok pc=${pc != null}")
        pc?.dispose()
        factory.dispose()
        setContent {
            SinuaView(
                pattern = "glowing",
                modifier = Modifier.size(200.dp),
                onFrame = {
                    if (!reported) {
                        reported = true
                        Log.i(TAG, "frame")
                    }
                },
            )
        }
    }

    private class NoOpObserver : PeerConnection.Observer {
        override fun onSignalingChange(s: PeerConnection.SignalingState?) {}
        override fun onIceConnectionChange(s: PeerConnection.IceConnectionState?) {}
        override fun onIceConnectionReceivingChange(b: Boolean) {}
        override fun onIceGatheringChange(s: PeerConnection.IceGatheringState?) {}
        override fun onIceCandidate(c: IceCandidate?) {}
        override fun onIceCandidatesRemoved(c: Array<out IceCandidate>?) {}
        override fun onAddStream(m: MediaStream?) {}
        override fun onRemoveStream(m: MediaStream?) {}
        override fun onDataChannel(d: DataChannel?) {}
        override fun onRenegotiationNeeded() {}
    }

    private companion object {
        const val TAG = "SinuaSmoke"
    }
}
