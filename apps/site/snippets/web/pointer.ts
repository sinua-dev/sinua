import { mount } from "@sinua/web";

// Pointer and touch scatter: the dots near the pointer move out of its way.
const canvas = document.querySelector<HTMLCanvasElement>("#orb")!;
const fx = mount(canvas, { pattern: "working", pointer: true });

// Turn it off again (it unbinds its listeners).
fx.update({ pointer: false });
