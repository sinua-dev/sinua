import { useState } from "react";
import { SinuaView } from "@sinua/web/react";
import type { AgentState } from "@sinua/core";

// No spec file: one pattern, and the agent's state moves it (idle breathes and dims,
// listening draws inward, thinking stirs, speaking swells).
export function AssistantOrb({ voice }: { voice?: import("@sinua/core").VoiceSource }) {
  const [state, setState] = useState<AgentState>("idle");
  // With a `voice` the view follows the source's state on its own; this is the manual way.
  void setState;
  return <SinuaView pattern="working" state={state} voice={voice} style={{ width: 160, height: 160 }} />;
}
