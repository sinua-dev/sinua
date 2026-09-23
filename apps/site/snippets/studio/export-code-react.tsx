import { SinuaView } from "@sinua/web/react";

export function Visual() {
  return <SinuaView pattern="working" overrides={{ glowRadius: 3, glowStrength: 0.6 }} style={{ width: 160, height: 160 }} />;
}

// Or use the file: Spec menu → orb-working.fxspec.json (File tab).
