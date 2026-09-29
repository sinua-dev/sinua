import { useEffect, useState } from "react";
import { SinuaOrb, createVoiceSource } from "@sinua/react-native";

// No microphone, no network, no permission: backed by the native simulator.
export function SimulatedConversation() {
  const [voice] = useState(() => createVoiceSource({ vendor: "simulated", sample: "barge-in" }));
  useEffect(() => {
    voice.connect().catch(console.error); // plays, and loops for the samples
    return () => voice.release();
  }, [voice]);
  return <SinuaOrb pattern="glowing" voice={voice} style={{ width: 160, height: 160 }} />;
}
