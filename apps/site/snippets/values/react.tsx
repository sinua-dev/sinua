import { SinuaView } from "@sinua/web/react";

// One value: how much of the ring is filled (0 to 1).
export function Upload({ fraction }: { fraction: number }) {
  return <SinuaView pattern="completing" overrides={{ progress: fraction }} style={{ width: 64, height: 64 }} />;
}

// One value per ring: `progress0` is the outermost. `ringCount` sets how many are drawn.
export function ActivityRings({ move, exercise, stand }: { move: number; exercise: number; stand: number }) {
  return (
    <SinuaView
      pattern="tracking"
      overrides={{ ringCount: 3, progress0: move, progress1: exercise, progress2: stand }}
      style={{ width: 96, height: 96 }}
    />
  );
}
