import { SinuaEdge } from "@sinua/web/components";
import type { VoiceSource } from "@sinua/core";

// A glow around the edge of the screen that follows the agent: hidden at rest, faint
// while listening, wide while speaking, a circling segment while thinking.
export function ScreenGlow({ voice }: { voice: VoiceSource }) {
  return (
    <SinuaEdge
      pattern="framing"
      voice={voice}
      cornerRadius={0.12} // match your screen's or container's corners
      label="" // decorative: the conversation is announced elsewhere
      style={{ position: "fixed", inset: 0, pointerEvents: "none", zIndex: 10 }}
    />
  );
}
