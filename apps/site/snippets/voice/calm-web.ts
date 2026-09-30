import { mount } from "@sinua/web";
import { LocalMicVoiceSource } from "@sinua/voice/mic";

const canvas = document.querySelector<HTMLCanvasElement>("#signal")!;
const voice = new LocalMicVoiceSource();

// The voice states add a glow and a pulse; your overrides go on top of them,
// in every state. Here: a softer glow and no pulse.
export const fx = mount(canvas, {
  pattern: "waveform",
  voice,
  overrides: { glowStrength: 0.15, pulseStrength: 0 },
});
