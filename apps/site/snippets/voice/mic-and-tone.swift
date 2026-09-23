import SinuaVoice

// The user's own mic, analysed on the device (no network). Needs NSMicrophoneUsageDescription.
let micVoice = LocalMicVoiceSource()

// A synthetic, speech-like signal: no mic, no network, silent. Good for previews and tests.
let toneVoice = TestToneVoiceSource()
