import { useEffect, useRef } from "react";
import type { FxHandle } from "@sinua/web";
import { SinuaView } from "@sinua/web/react";
import activity from "../spec/activity-rules.fxspec.json";

// Your numbers go in; the file decides what they mean. A binding moves the progress, a rule
// switches to goalReached at 10,000 steps, and one call plays a short celebration on top.
export function DailyActivity({ steps, heartRate }: { steps: number; heartRate: number }) {
  const fx = useRef<FxHandle | null>(null);
  const last = useRef(steps);
  useEffect(() => {
    if (last.current < 10_000 && steps >= 10_000) fx.current?.trigger("celebrate");
    last.current = steps;
  }, [steps]);
  return <SinuaView spec={activity} inputs={{ steps, heartRate }} onReady={(h) => (fx.current = h)} style={{ width: 160, height: 160 }} />;
}
