import { useEffect, useState } from "react";
import { Button, View } from "react-native";
import { SinuaOrb, createVoiceSource, type AgentState } from "@sinua/react-native";

// The source is native (the same one iOS and Android ship); your app owns it.
export function TalkingOrb() {
  const [voice] = useState(() => createVoiceSource({ vendor: "mic" }));
  const [state, setState] = useState<AgentState>("idle");

  useEffect(() => {
    const off = voice.onStateChange(setState);
    return () => {
      off();
      voice.release(); // disconnects and drops the native source
    };
  }, [voice]);

  return (
    <View>
      {/* The view binds the source; it never connects it. */}
      <SinuaOrb pattern="speaking" voice={voice} style={{ width: 160, height: 160 }} />
      {/* Connect from a user action: the mic prompt and audio playback need one. */}
      <Button title={state === "idle" ? "Talk" : state} onPress={() => voice.connect().catch(console.error)} />
    </View>
  );
}
