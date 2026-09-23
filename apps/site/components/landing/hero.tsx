"use client";
/**
 * The hero: the product's own subject, running. The visual on the left is the
 * engine, and the four buttons under it are the states a voice agent moves
 * through -- clicking one shows the visual in that state (families' profile),
 * which is the page's one interactive moment.
 *
 * Every string comes from lib/brand.ts; the copy there is GTM's to replace.
 */
import { useEffect, useState } from "react";
import Link from "next/link";
import { brand } from "@/lib/brand";
import { VOICE_STATES, type VoiceState } from "@/lib/voice-state";
import { LiveVisual } from "./live-visual";

/** A level that rises and falls, so listening and speaking read as sound without any audio device. */
function useSimulatedLevel(active: boolean) {
  const [level, setLevel] = useState(0.35);
  useEffect(() => {
    if (!active) return;
    let raf = 0;
    const start = performance.now();
    const tick = (t: number) => {
      const s = (t - start) / 1000;
      setLevel(0.35 + 0.3 * Math.abs(Math.sin(s * 1.6)) + 0.12 * Math.sin(s * 5.3));
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
    return () => cancelAnimationFrame(raf);
  }, [active]);
  return level;
}

export function Hero() {
  const [state, setState] = useState<VoiceState>("listening");
  const level = useSimulatedLevel(state === "listening" || state === "speaking");

  return (
    <section className="lp-section lp-hero">
      <div className="lp-hero-text">
        <h1 className="lp-h1">{brand.copy.tagline}</h1>
        <p className="lp-lead">{brand.copy.heroLead}</p>
        <div className="lp-actions">
          <code className="lp-install">npm i {brand.packages.web}</code>
          <Link className="lp-link" href={brand.links.docs}>
            Read the docs
          </Link>
          <Link className="lp-link" href={brand.links.gallery}>
            See every pattern
          </Link>
        </div>
      </div>

      <figure className="lp-hero-visual">
        <LiveVisual pattern="glowing" state={state} level={level} label={`A voice agent ${state}`} className="lp-orb" />
        <figcaption className="lp-states" role="radiogroup" aria-label="Agent state">
          {VOICE_STATES.map((s) => (
            <button key={s} type="button" role="radio" aria-checked={state === s} onClick={() => setState(s)} className="lp-state" data-on={state === s}>
              {s}
            </button>
          ))}
        </figcaption>
      </figure>
    </section>
  );
}
