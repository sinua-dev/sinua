import { mount } from "@sinua/web";
import { SimulatedVoiceSource } from "@sinua/core/dev";

const canvas = document.querySelector<HTMLCanvasElement>("#orb")!;
const caption = document.querySelector<HTMLParagraphElement>("#caption")!;

// A believable conversation with no microphone and no network: for demos, previews and tests.
// Samples: "calendar", "quick-answer", "long-answer", "barge-in" (or pass your own script).
export const voice = new SimulatedVoiceSource("barge-in");
export const fx = mount(canvas, { pattern: "glowing", voice });

// Captions: the current line, as much of it as has been "said".
voice.onFrame((f) => {
  caption.textContent = f.line.slice(0, f.shown);
});

// Plays, and loops for the samples. No permission prompt: nothing is recorded.
await voice.connect();
