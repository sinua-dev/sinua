import { createVoiceSource } from "@sinua/react-native";

// The device mic, analysed natively (no network). Needs the platform's mic permission.
export const mic = createVoiceSource({ vendor: "mic" });

// A synthetic, speech-like signal: no mic, no network, silent (its output gain is zero).
export const tone = createVoiceSource({ vendor: "test" });
