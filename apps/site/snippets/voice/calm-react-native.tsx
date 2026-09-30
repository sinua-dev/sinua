import { useState } from "react";
import { SinuaView, createVoiceSource } from "@sinua/react-native";

export function CalmSignal() {
  const [voice] = useState(() => createVoiceSource({ vendor: "mic" }));
  // The voice states add a glow and a pulse; your overrides go on top of them,
  // in every state. Here: a softer glow and no pulse.
  return (
    <SinuaView
      pattern="waveform"
      overrides={{ glowStrength: 0.15, pulseStrength: 0 }}
      voice={voice}
      style={{ width: 220, height: 120 }}
    />
  );
}
