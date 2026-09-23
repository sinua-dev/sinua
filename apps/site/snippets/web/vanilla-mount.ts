import { mount } from "@sinua/web";
import voiceOrb from "../spec/voice-orb.fxspec.json";

// Any <canvas>: mount() sizes it to its CSS box and runs the render loop.
const canvas = document.querySelector<HTMLCanvasElement>("#orb")!;
const fx = mount(canvas, { spec: voiceOrb, state: "idle" });

// Change it later: only what you pass changes.
fx.update({ state: "listening", inputs: { micMuted: 0 } });

// Stop the loop and release the canvas when the view goes away.
fx.destroy();
