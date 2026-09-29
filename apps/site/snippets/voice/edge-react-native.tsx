import { StyleSheet, View } from "react-native";
import { SinuaEdge, type VoiceSourceHandle } from "@sinua/react-native";

export function ScreenGlow({ voice }: { voice: VoiceSourceHandle }) {
  return (
    <View style={StyleSheet.absoluteFill} pointerEvents="none">
      <SinuaEdge pattern="framing" voice={voice} cornerRadius={0.12} style={StyleSheet.absoluteFill} />
    </View>
  );
}
