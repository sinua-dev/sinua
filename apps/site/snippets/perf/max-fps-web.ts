import { mount } from "@sinua/web";

const canvas = document.querySelector<HTMLCanvasElement>("#orb")!;

// Cap the frame rate (the clock keeps wall time, so the motion's speed is unchanged).
export const fx = mount(canvas, {
  pattern: "breathing",
  maxFps: 24,
  onFrame: ({ dtMs, computeMs, paintMs }) => {
    if (computeMs + paintMs > 8) console.debug("slow frame", { dtMs, computeMs, paintMs });
  },
});
