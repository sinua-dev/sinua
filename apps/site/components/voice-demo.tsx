"use client";
/**
 * <VoiceDemo> -- a view driven by the engine's simulated conversation, the way an
 * app wires one: `mount(canvas, { voice })` with a `SimulatedVoiceSource`. No
 * microphone, no audio, no network: the site's Permissions-Policy keeps the mic off.
 *
 *   <VoiceDemo sample="barge-in" />             plays a sample, with its captions
 *   <VoiceDemo button />                        adds <sinua-voice-button> on the same source
 *   <VoiceDemo effects pattern="tracking" />    a view with the one-shot effect triggers
 *
 * Like <Demo>, the engine is imported only when a page shows one.
 */
import { useEffect, useRef, useState } from "react";
import type { FxHandle } from "@sinua/web";
import type { SimulatedVoiceSource } from "@sinua/core";
import { useSiteTheme } from "@/lib/use-site-theme";

const SAMPLES = ["calendar", "quick-answer", "long-answer", "barge-in"] as const;
const EFFECTS = ["success", "error", "celebrate"] as const;

export interface VoiceDemoProps {
  pattern?: string;
  sample?: (typeof SAMPLES)[number];
  /** Show the sample picker. */
  samples?: boolean;
  /** Add a voice button on the same source; the conversation starts when it's pressed. */
  button?: boolean;
  /** Show the one-shot effect buttons (no conversation). */
  effects?: boolean;
}

export function VoiceDemo({ pattern = "glowing", sample = "calendar", samples, button, effects }: VoiceDemoProps) {
  const canvas = useRef<HTMLCanvasElement>(null);
  const slot = useRef<HTMLDivElement>(null);
  const handle = useRef<FxHandle | null>(null);
  const voiceRef = useRef<SimulatedVoiceSource | null>(null);
  const theme = useSiteTheme();
  const [picked, setPicked] = useState(sample);
  const [caption, setCaption] = useState("");
  const [state, setState] = useState("idle");
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let live = true;
    let buttonEl: HTMLElement | null = null;
    (async () => {
      const [{ mount }, { SimulatedVoiceSource }] = await Promise.all([import("@sinua/web"), import("@sinua/core")]);
      if (!live || !canvas.current) return;
      const voice = effects ? null : new SimulatedVoiceSource(picked);
      voiceRef.current = voice;
      voice?.onFrame((f) => {
        setCaption(f.line.slice(0, f.shown));
        setState(String(f.state));
      });
      handle.current = mount(canvas.current, {
        pattern,
        theme,
        label: effects ? "Coach" : "Assistant",
        ...(voice ? { voice } : {}),
        onError: (d) => setError(d.map((x) => x.message).join("; ")),
      });
      if (voice && button && slot.current) {
        const { defineSinuaVoiceButtonElement } = await import("@sinua/web/voice-button");
        defineSinuaVoiceButtonElement();
        if (!live) return;
        buttonEl = document.createElement("sinua-voice-button");
        (buttonEl as HTMLElement & { source: unknown }).source = voice;
        buttonEl.setAttribute("theme", theme === "dark" ? "dark" : "light");
        slot.current.replaceChildren(buttonEl);
      } else if (voice) {
        await voice.connect(); // plays and loops; the view pauses it off screen
      }
    })().catch((e: unknown) => live && setError(e instanceof Error ? e.message : String(e)));
    return () => {
      live = false;
      voiceRef.current?.disconnect();
      voiceRef.current = null;
      handle.current?.destroy();
      handle.current = null;
      buttonEl?.remove();
    };
  }, [pattern, picked, theme, button, effects]);

  const pill = (on: boolean) =>
    `rounded border px-1.5 py-0.5 text-xs ${on ? "border-fd-primary text-fd-primary" : "border-fd-border text-fd-muted-foreground"}`;

  return (
    <figure className="fx-demo not-prose my-4 flex flex-col items-center gap-2 rounded-xl border border-fd-border bg-fd-card p-3">
      <canvas ref={canvas} className="aspect-square w-full max-w-40" />
      {effects ? (
        <div className="flex flex-wrap justify-center gap-1" aria-label="Play an effect">
          {EFFECTS.map((e) => (
            <button key={e} type="button" className={pill(false)} onClick={() => handle.current?.trigger(e)}>
              {e}
            </button>
          ))}
        </div>
      ) : (
        <>
          <figcaption className="min-h-5 text-center text-sm text-fd-muted-foreground" aria-live="off">
            <span className="font-medium text-fd-foreground">{state}</span>
            {caption ? ` · ${caption}` : ""}
          </figcaption>
          {button ? <div ref={slot} className="flex justify-center" /> : null}
          {samples ? (
            <div role="radiogroup" aria-label="Sample" className="flex flex-wrap justify-center gap-1">
              {SAMPLES.map((s) => (
                <button key={s} type="button" role="radio" aria-checked={picked === s} className={pill(picked === s)} onClick={() => setPicked(s)}>
                  {s}
                </button>
              ))}
            </div>
          ) : null}
          <span className="text-[11px] text-fd-muted-foreground">A simulated conversation: no microphone, no audio, no network.</span>
        </>
      )}
      {error ? <span className="text-[11px] text-fd-muted-foreground">{error}</span> : null}
    </figure>
  );
}
