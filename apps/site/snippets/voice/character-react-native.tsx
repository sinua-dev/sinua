import { SinuaCharacter, type VoiceSourceHandle } from "@sinua/react-native";

// It looks at you while you talk, looks away while it thinks, and its mouth follows
// the agent's voice. `hue` turns the shell; the eyes keep their colour.
export function Assistant({ voice }: { voice: VoiceSourceHandle }) {
  return (
    <SinuaCharacter
      pattern="buzzy"
      voice={voice}
      hue={190}
      accessibilityLabel="Buzzy"
      labels={{ listening: "Buzzy is listening", speaking: "Buzzy is speaking" }}
      style={{ width: 160, height: 160 }}
    />
  );
}
