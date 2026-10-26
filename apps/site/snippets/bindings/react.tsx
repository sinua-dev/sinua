import { SinuaView } from "@sinua/web/react";
import activity from "../spec/activity-rings.fxspec.json";

// The mapping lives in the spec's `bindings`; the app passes only its raw numbers.
// An input you leave out keeps its target at the design's value (reported as inactive).
export function DailyActivity({ steps, activeMinutes, heartRate }: { steps: number; activeMinutes: number; heartRate: number }) {
  return <SinuaView spec={activity} inputs={{ steps, activeMinutes, heartRate }} style={{ width: 120, height: 120 }} />;
}
