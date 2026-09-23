import { SinuaView } from "@sinua/web/react";
import voiceOrb from "../spec/voice-orb.fxspec.json";

// A pattern on its own: size the box with CSS, the canvas follows it.
export function Breathing() {
  return <SinuaView pattern="breathing" style={{ width: 160, height: 160 }} />;
}

// A spec from the Studio: `state` picks its lifecycle state, `inputs` feeds its bindings.
export function Assistant({ state, muted }: { state: "idle" | "listening" | "thinking" | "speaking"; muted: boolean }) {
  return <SinuaView spec={voiceOrb} state={state} inputs={{ micMuted: muted ? 1 : 0 }} style={{ width: 160, height: 160 }} />;
}
