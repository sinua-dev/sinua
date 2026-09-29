import { useState } from "react";
import { SinuaView, createVoiceSource } from "@sinua/react-native";

export function CalmSignal() {
  const [voice] = useState(() => createVoiceSource({ vendor: "mic" }));
  // The voice states add particles, glow and pulse; your overrides go on top of them,
  // in every state. Here: no particles, a softer glow, no pulse.
  return (
    <SinuaView
      pattern="waveform"
      overrides={{ particleStrength: 0, glowStrength: 0.15, pulseStrength: 0 }}
      voice={voice}
      style={{ width: 220, height: 120 }}
    />
  );
}
