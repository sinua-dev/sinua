import { useEffect, useState } from "react";
import { Text, View } from "react-native";
import { SinuaOrb, createVoiceSource } from "@sinua/react-native";

// The simulated conversation sends its lines as they are said, so captions work offline.
export function CaptionedConversation() {
  const [voice] = useState(() => createVoiceSource({ vendor: "simulated", sample: "barge-in" }));
  const [line, setLine] = useState("");
  useEffect(() => {
    const stop = voice.onTranscript((u) => setLine(u.text)); // the whole turn so far
    voice.connect().catch(console.error);
    return () => {
      stop();
      voice.release();
    };
  }, [voice]);
  return (
    <View>
      <SinuaOrb pattern="glowing" voice={voice} style={{ width: 160, height: 160 }} />
      <Text>{line}</Text>
    </View>
  );
}
