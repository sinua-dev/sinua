import { useEffect, useRef } from "react";
import type { FxHandle } from "@sinua/web";
import { SinuaView } from "@sinua/web/react";
import rings from "../spec/activity-rules.fxspec.json";

// Your numbers go in; the file decides what they mean. A binding fills the rings, a rule
// switches to goalReached at 10,000 steps, and one call plays a short celebration on top.
export function DailyRings({ steps, heartRate }: { steps: number; heartRate: number }) {
  const fx = useRef<FxHandle | null>(null);
  const last = useRef(steps);
  useEffect(() => {
    if (last.current < 10_000 && steps >= 10_000) fx.current?.trigger("celebrate");
    last.current = steps;
  }, [steps]);
  return <SinuaView spec={rings} inputs={{ steps, heartRate }} onReady={(h) => (fx.current = h)} style={{ width: 160, height: 160 }} />;
}
