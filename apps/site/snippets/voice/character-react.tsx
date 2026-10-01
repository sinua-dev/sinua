import { SinuaCharacter } from "@sinua/web/components";
import type { VoiceSource } from "@sinua/core";

// A character for your assistant: it looks at you while you talk, looks away while it
// thinks, and its mouth follows the agent's voice.
export function Assistant({ voice }: { voice: VoiceSource }) {
  return (
    <SinuaCharacter
      pattern="buzzy"
      voice={voice}
      hue={190} // turns the shell; the eyes and accents keep their colours
      label="Buzzy"
      labels={{ listening: "Buzzy is listening", speaking: "Buzzy is speaking" }}
      style={{ width: 160, height: 160 }}
    />
  );
}
