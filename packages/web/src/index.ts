// @sinua/web -- the Web renderer: the paint contract (`drawFrame`, shared
// with the Studio) and a framework-agnostic `mount(canvas, options)` that
// runs the whole loop. React users: `@sinua/web/react`'s `<SinuaView/>`.
// See docs/fx-view.md.
export { drawFrame, drawCrossDissolve, drawPacked, ink } from "./paint.js";
export type { FxFill, FxFillGradient, FxGradientStop, FxEffectRun, PaintFrame } from "./paint.js";
export { mount, defaultVoiceOptions, DPR_CAP, REDUCED_MOTION_T } from "./mount.js";
export type { SinuaViewOptions, FxHandle, FxFrameStats } from "./mount.js";
export { createFramePacer, performanceFor, watchLowBattery, DEFAULT_LOW_POWER } from "./perf.js";
export type { FxPerformance } from "./perf.js";
