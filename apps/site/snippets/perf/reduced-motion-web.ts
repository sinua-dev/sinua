import { mount } from "@sinua/web";

const canvas = document.querySelector<HTMLCanvasElement>("#orb")!;

// "auto" (default) follows prefers-reduced-motion: a still pose instead of motion.
// With a voice attached it still redraws (up to 30 Hz): the voice cue is information.
export const fx = mount(canvas, { pattern: "working", reducedMotion: "auto" });

// "always" / "never" override the system setting, e.g. from an in-app preference:
fx.update({ reducedMotion: "always" });
