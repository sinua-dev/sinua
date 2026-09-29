"use client";
/**
 * App data, not voice: the docs' activity-rings file (snippets/spec/activity-rules.fxspec.json)
 * mounted as it is. The sliders are the app's numbers; the file's bindings fill the rings, its
 * rules pick the state, and crossing the step goal plays a one-shot effect -- the React snippet
 * beside it (snippets/states/goal-react.tsx) is the same thing in an app. The state shown is
 * the engine's own `fxSpecDeriveState`, the function the view itself runs.
 */
import { useEffect, useMemo, useRef, useState } from "react";
import type { FxHandle } from "@sinua/web";
import { PgSlider, Snippet } from "@sinua/design";
import { useSiteTheme } from "@/lib/use-site-theme";

const GOAL = 10_000;
const EFFECTS = ["success", "error", "celebrate"] as const;
type Derive = (json: string, inputs: Record<string, number>, previous: string | null) => string | null;

export function Goal({ spec, react }: { spec: string; react: string }) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const handle = useRef<FxHandle | null>(null);
  const derive = useRef<Derive | null>(null);
  const theme = useSiteTheme();
  const [steps, setSteps] = useState(6500);
  const [heartRate, setHeartRate] = useState(90);
  const [state, setState] = useState<string | null>(null);
  const inputs = useMemo(() => ({ steps, heartRate }), [steps, heartRate]);

  // Mount reads the latest options: the engine chunk can land after a theme or input change.
  const latest = useRef({ inputs, theme });
  latest.current = { inputs, theme };

  useEffect(() => {
    let live = true;
    Promise.all([import("@sinua/web"), import("@sinua/core")])
      .then(([{ mount }, core]) => {
        if (!live || !canvas.current) return;
        derive.current = core.fxSpecDeriveState;
        handle.current = mount(canvas.current, { spec: JSON.parse(spec) as object, ...latest.current, label: "Daily rings" });
        setState(core.fxSpecDeriveState(spec, latest.current.inputs, null));
      })
      .catch((err: unknown) => {
        console.error("Sinua: the engine chunk failed to load; the %s will stay blank.", "goal demo", err);
      });
    return () => {
      live = false;
      handle.current?.destroy();
      handle.current = null;
    };
  }, [spec]);

  useEffect(() => {
    handle.current?.update({ inputs, theme });
    // The previous state goes back in, as the view does: that is what makes hysteresis hold.
    setState((prev) => derive.current?.(spec, inputs, prev) ?? null);
  }, [inputs, theme, spec]);

  const onSteps = (next: number) => {
    if (steps < GOAL && next >= GOAL) handle.current?.trigger("celebrate");
    setSteps(next);
  };

  return (
    <section className="lp-section" aria-labelledby="lp-goal">
      <header className="lp-section-head">
        <h2 id="lp-goal" className="lp-h2">
          Your numbers pick the state
        </h2>
        <p className="lp-lead">
          Pass the app&apos;s values and let the file decide what they mean. Here a binding fills the rings, a rule switches to the goal state at 10,000
          steps, and one call plays a short celebration on top. Push the heart rate past 150 for the other rule.
        </p>
      </header>

      <div className="lp-workbench">
        <div className="lp-stage lp-look-stage">
          <canvas ref={canvas} className="lp-look-visual" aria-label="Daily rings" />
        </div>
        <div className="lp-inspector">
          <PgSlider label="Steps" value={steps} min={0} max={15000} step={250} display={steps.toLocaleString("en-US")} onChange={onSteps} />
          <PgSlider label="Heart rate" value={heartRate} min={60} max={170} step={1} display={`${heartRate} bpm`} onChange={setHeartRate} />
          <div className="pg-field">
            <span className="pg-label">
              <span>State</span>
            </span>
            <p className="lp-goal-state">
              <code>{state ?? "base"}</code>
              <span>{state === "goalReached" ? "the step rule holds" : state === "intense" ? "above 150 bpm, back below 145" : "no rule holds"}</span>
            </p>
          </div>
          <div className="pg-field">
            <span className="pg-label">
              <span>Play an effect</span>
            </span>
            <div className="lp-toggles">
              {EFFECTS.map((e) => (
                <button key={e} type="button" className="pg-tab" onClick={() => handle.current?.trigger(e)}>
                  {e}
                </button>
              ))}
            </div>
          </div>
        </div>
        <div className="lp-workbench-code">
          <Snippet
            tabs={[
              { id: "react", label: "React", code: react },
              { id: "spec", label: "activity-rules.fxspec.json", code: spec },
            ]}
          />
        </div>
      </div>
    </section>
  );
}
