package snippets.voice

import dev.sinua.voice.LocalMicVoiceSource
import dev.sinua.voice.TestToneVoiceSource

// The user's own mic, analysed on the device (no network). Needs RECORD_AUDIO.
val micVoice = LocalMicVoiceSource()

// A synthetic, speech-like signal: no mic, no network, silent. Good for previews and tests.
val toneVoice = TestToneVoiceSource()
