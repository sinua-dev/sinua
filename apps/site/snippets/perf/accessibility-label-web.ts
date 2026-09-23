import { mount } from "@sinua/web";

const canvas = document.querySelector<HTMLCanvasElement>("#orb")!;

// The canvas gets role="img" and this name. Default: the spec's `name`, else the pattern.
export const fx = mount(canvas, { pattern: "listening", label: "Assistant is listening" });

// Purely decorative (text next to it already says it): an empty label hides it.
fx.update({ label: "" });
