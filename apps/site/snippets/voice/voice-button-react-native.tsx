import { useEffect, useState } from "react";
import { View } from "react-native";
import { SinuaOrb, SinuaVoiceButton, createVoiceSource } from "@sinua/react-native";

export function AssistantControls() {
  const [voice] = useState(() => createVoiceSource({ vendor: "mic" }));
  useEffect(() => () => voice.release(), [voice]);
  return (
    <View>
      <SinuaOrb pattern="glowing" voice={voice} style={{ width: 160, height: 160 }} />
      {/* The same handle for both: the button drives the session, the view shows it. */}
      <SinuaVoiceButton voice={voice} mode="pushToTalk" />
    </View>
  );
}
