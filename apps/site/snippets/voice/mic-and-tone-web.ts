import { LocalMicVoiceSource } from "@sinua/voice/mic";
import { TestToneVoiceSource } from "@sinua/voice/tone";

// The user's own mic, analysed on the device (no network).
export const mic = new LocalMicVoiceSource();

// A synthetic, speech-like signal: no mic, no network, silent. Good for demos and tests.
export const tone = new TestToneVoiceSource();
