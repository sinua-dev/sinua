"use client";
/**
 * <Demo> -- the real engine in the docs, the way an app uses it (`mount` from
 * @sinua/web). As families' pages write it:
 *   <Demo object="orb" pattern="breathing" />
 *   <Demo spec="spec/voice-orb.fxspec.json" controls={[{ state: [...] }, { input: "micLevel", min: 0, max: 1 }]} />
 * `spec` is a path under apps/site/snippets/, copied to public/ by scripts/copy-specs.mjs.
 *
 * The engine module is imported dynamically, so a page without a demo never
 * loads it. `mount` itself handles the DPR cap, pausing off screen or in a
 * hidden tab, and reduced motion.
 */
import { useEffect, useRef, useState } from "react";
import type { FxHandle, SinuaViewOptions } from "@sinua/web";
import { useSiteTheme } from "@/lib/use-site-theme";

export type DemoControl =
  | { state: string[] }
  | { input: string; min?: number; max?: number }
  | { value: string; min?: number; max?: number };

export interface DemoProps {
  object?: string;
  pattern?: string;
  spec?: string;
  controls?: DemoControl[];
}

const isState = (c: DemoControl): c is { state: string[] } => "state" in c;
const inputName = (c: DemoControl) => ("input" in c ? c.input : "value" in c ? c.value : "");

export function Demo({ object, pattern, spec, controls }: DemoProps) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const handle = useRef<FxHandle | null>(null);
  const theme = useSiteTheme();
  const [state, setState] = useState<string | undefined>(undefined);
  const [inputs, setInputs] = useState<Record<string, number>>({});
  const [error, setError] = useState<string | null>(null);

  const title = spec ? spec.split("/").pop()!.replace(/\.fxspec\.json$/, "") : [object, pattern].filter(Boolean).join(" · ");

  // One mount per demo; later prop and control changes go through `update`.
  useEffect(() => {
    let live = true;
    (async () => {
      const [{ mount }, doc] = await Promise.all([
        import("@sinua/web"),
        // `r.json()` is `any`; keep it `unknown` here and let the engine's own
        // resolver judge the shape -- a bad spec comes back as diagnostics
        // through `onError`, which is where the reader should see it anyway.
        spec
          ? fetch(`/${spec}`).then((r): Promise<unknown> => (r.ok ? r.json() : Promise.reject(new Error(`${r.status} ${spec}`))))
          : Promise.resolve(null),
      ]);
      if (!live || !canvas.current) return;
      handle.current = mount(canvas.current, {
        ...(doc ? { spec: doc } : { pattern: pattern ?? "breathing" }),
        theme,
        // `FxDiagnostic` always carries `message`; the old fallback stringified
        // the object itself, which renders "[object Object]" to the reader.
        onError: (diagnostics) => setError(diagnostics.map((d) => d.message).join("; ")),
      } satisfies SinuaViewOptions);
    })().catch((e: unknown) => live && setError(e instanceof Error ? e.message : String(e)));
    return () => {
      live = false;
      handle.current?.destroy();
      handle.current = null;
    };
  }, [spec, pattern, theme]);

  useEffect(() => {
    handle.current?.update({ state, inputs });
  }, [state, inputs]);

  const stateControl = controls?.find(isState);
  const sliders = (controls ?? []).filter((c): c is Exclude<DemoControl, { state: string[] }> => !isState(c));

  return (
    <figure className="fx-demo not-prose my-4 flex flex-col items-center gap-2 rounded-xl border border-fd-border bg-fd-card p-3">
      <canvas ref={canvas} className="aspect-square w-full max-w-40" aria-label={`${title} demo`} />
      <figcaption className="text-sm font-medium text-fd-foreground">
        {pattern && !spec ? <a href={`#${pattern}`}>{title}</a> : title}
      </figcaption>
      {stateControl ? (
        <div role="radiogroup" aria-label="State" className="flex flex-wrap justify-center gap-1">
          {[undefined, ...stateControl.state].map((s) => (
            <button
              key={s ?? "base"}
              type="button"
              role="radio"
              aria-checked={state === s}
              onClick={() => setState(s)}
              className={`rounded border px-1.5 py-0.5 text-xs ${state === s ? "border-fd-primary text-fd-primary" : "border-fd-border text-fd-muted-foreground"}`}
            >
              {s ?? "base"}
            </button>
          ))}
        </div>
      ) : null}
      {sliders.map((c) => {
        const name = inputName(c);
        const min = c.min ?? 0;
        const max = c.max ?? 1;
        return (
          <label key={name} className="flex w-full max-w-60 items-center gap-2 text-xs text-fd-muted-foreground">
            <span className="w-24 shrink-0 truncate">{name}</span>
            <input
              type="range"
              className="w-full"
              min={min}
              max={max}
              step={(max - min) / 100}
              value={inputs[name] ?? min}
              onChange={(e) => setInputs((p) => ({ ...p, [name]: Number(e.target.value) }))}
            />
            <span className="w-10 shrink-0 text-right tabular-nums">{Math.round(((inputs[name] ?? min) + Number.EPSILON) * 100) / 100}</span>
          </label>
        );
      })}
      {error ? <span className="text-[11px] text-fd-muted-foreground">{error}</span> : null}
    </figure>
  );
}
