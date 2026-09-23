import { SinuaOrb, type AgentState, type VoiceSourceHandle } from "@sinua/react-native";

// No spec file: one pattern, and the agent's state moves it.
export function PlainAssistantOrb({ voice, state }: { voice?: VoiceSourceHandle; state?: AgentState }) {
  // With a voice handle the native view follows its state; `state` overrides it.
  return <SinuaOrb pattern="working" state={state} voice={voice} style={{ width: 160, height: 160 }} />;
}
