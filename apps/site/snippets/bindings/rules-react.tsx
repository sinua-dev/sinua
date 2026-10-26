import { SinuaView } from "@sinua/web/react";
import activity from "../spec/activity-rules.fxspec.json";

// The app passes numbers; the file's `rules` pick the state: goalReached at 10,000 steps,
// intense above 150 bpm (and back below 145). With no rule holding, the base design shows.
export function DailyActivity({ steps, heartRate }: { steps: number; heartRate: number }) {
  return <SinuaView spec={activity} inputs={{ steps, heartRate }} style={{ width: 120, height: 120 }} />;
}
