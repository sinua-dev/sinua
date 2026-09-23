import { mount, watchLowBattery } from "@sinua/web";

const canvas = document.querySelector<HTMLCanvasElement>("#orb")!;

// Low power: the spec's `performance.lowPower` block if it has one, else 30 fps with
// glow and particles off. The Web has no reliable OS signal, so the app decides:
export const fx = mount(canvas, { pattern: "speaking", lowPower: false });

// ... e.g. from the Battery API where it exists (Chromium): low below 20% and unplugged.
void watchLowBattery((low) => fx.update({ lowPower: low }));
