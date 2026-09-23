import { bindReactiveInput, frameWithOverrides } from "@sinua/core";

declare const steps: number; // your app's value
declare const t: number; // seconds since the view started

// 10,000 steps fills the ring, eased out so the last few thousand feel slower.
const overrides = bindReactiveInput({
  value: steps,
  target: "progress",
  input: [0, 10_000],
  curve: "easeOut",
});
const frame = frameWithOverrides("completing", 64, t, overrides);
