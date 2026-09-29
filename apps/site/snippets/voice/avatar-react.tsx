import { SinuaAvatar } from "@sinua/web/react";
import type { VoiceSource } from "@sinua/core";

// A participant's picture with a ring that shows who is talking: it thickens outward
// with the voice and never covers the face. The ring is decorative; `alt` names the person.
export function Speaker({ voice }: { voice: VoiceSource }) {
  return <SinuaAvatar src="/ada.jpg" alt="Ada" voice={voice} style={{ width: 56 }} />;
}
