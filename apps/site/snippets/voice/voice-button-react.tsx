import { useState } from "react";
import { SinuaView, SinuaVoiceButton } from "@sinua/web/react";
import { LocalMicVoiceSource } from "@sinua/voice/mic";

export function Assistant() {
  const [source] = useState(() => new LocalMicVoiceSource());
  return (
    <>
      <SinuaView pattern="glowing" voice={source} style={{ width: 160, height: 160 }} />
      {/* Press to connect; then press to mute and unmute. Long-press ends the session. */}
      <SinuaVoiceButton source={source} mode="toggle" />
    </>
  );
}
