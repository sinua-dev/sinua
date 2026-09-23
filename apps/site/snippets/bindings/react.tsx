import { SinuaView } from "@sinua/web/react";
import rings from "../spec/activity-rings.fxspec.json";

// The mapping lives in the spec's `bindings`; the app passes only its raw numbers.
// An input you leave out keeps its target at the design's value (reported as inactive).
export function DailyRings({ steps, waterMl, activeMinutes }: { steps: number; waterMl: number; activeMinutes: number }) {
  return <SinuaView spec={rings} inputs={{ steps, waterMl, activeMinutes }} style={{ width: 120, height: 120 }} />;
}
