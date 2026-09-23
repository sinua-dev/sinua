import { SinuaView } from "@sinua/web/react";
import type { VoiceSource } from "@sinua/core";
import voiceOrb from "../spec/voice-orb.fxspec.json";

// The spec's `states` hold one look per lifecycle state. With a voice attached, the
// view follows the agent's state ("listening", "thinking", "speaking", ...) on its own;
// pass `state` to drive it yourself instead.
export function AssistantOrb({ voice, state }: { voice?: VoiceSource; state?: string }) {
  return <SinuaView spec={voiceOrb} voice={voice} state={state} style={{ width: 160, height: 160 }} />;
}
