import { resolveFxSpec } from "@sinua/core";
import spec from "../spec/voice-orb.fxspec.json";

// Resolve once to see what the engine will draw for a lifecycle state.
const r = resolveFxSpec(JSON.stringify(spec), {
  state: "listening",
  inputs: { micLevel: 0.4, micMuted: 0 },
});
if (!r.ok) console.error(r.diagnostics);
console.log(r.state, r.overrides); // "listening" pattern, with audioLevel bound to micLevel
