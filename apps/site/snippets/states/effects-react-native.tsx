import { useState } from "react";
import { Button, View } from "react-native";
import { SinuaView } from "@sinua/react-native";

export function Coach() {
  const [goals, setGoals] = useState(0);
  return (
    <View>
      {/* Plays once each time `key` changes. */}
      <SinuaView pattern="tracking" effect={{ name: "celebrate", key: goals }} style={{ width: 160, height: 160 }} />
      <Button title="Log workout" onPress={() => setGoals((n) => n + 1)} />
    </View>
  );
}
