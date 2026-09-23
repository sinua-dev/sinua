import { View } from "react-native";
import { SinuaOrb, SinuaRing, SinuaView } from "@sinua/react-native";

export function AssistantCard({ specJson }: { specJson: string }) {
  return (
    <View style={{ gap: 24 }}>
      {/* Typed: one component per object, parameters as props. */}
      <SinuaRing pattern="completing" progress={0.65} strokeWidth={0.12} style={{ width: 64, height: 64 }} />
      {/* A spec file; `state` picks its lifecycle state. */}
      <SinuaOrb spec={specJson} state="listening" style={{ width: 160, height: 160 }} />
      {/* The low-level view: native test tone or mic drive it without a vendor. */}
      <SinuaView pattern="speaking" voice="test" style={{ width: 96, height: 96 }} />
    </View>
  );
}
