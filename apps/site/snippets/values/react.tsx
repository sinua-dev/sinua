import { SinuaView } from "@sinua/web/react";

// One value: how much is done (0 to 1); the done part of the dune glows.
export function Upload({ fraction }: { fraction: number }) {
  return <SinuaView pattern="dune" overrides={{ progress: fraction }} style={{ width: 160, height: 64 }} />;
}
