import { mount } from "@sinua/web";
import { LocalMicVoiceSource } from "@sinua/voice/mic";

const canvas = document.querySelector<HTMLCanvasElement>("#signal")!;
const voice = new LocalMicVoiceSource();

// The voice states add particles, glow and pulse; your overrides go on top of them,
// in every state. Here: no particles, a softer glow, no pulse.
export const fx = mount(canvas, {
  pattern: "waveform",
  voice,
  overrides: { particleStrength: 0, glowStrength: 0.15, pulseStrength: 0 },
});
