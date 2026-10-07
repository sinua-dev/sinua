"use client";
/**
 * The hero is a stage, like the Studio's: one large live visual in a simulated
 * voice conversation. The script runs on its own; picking a state takes over,
 * and "Auto" hands it back. The swatches recolor the same frame live.
 *
 * Every string comes from lib/brand.ts.
 */
import { useRef, useState } from "react";
import Link from "next/link";
import { Icon } from "@sinua/design";
import { brand } from "@/lib/brand";
import { SCRIPT, useConversation } from "@/lib/conversation";
import { VOICE_STATES, type VoiceState } from "@/lib/voice-state";
import { LiveVisual } from "./live-visual";
import { SWATCHES, swatchOverrides } from "./swatches";

export function Hero({ patternCount }: { patternCount: number }) {
  const [hold, setHold] = useState<VoiceState | null>(null);
  const [swatch, setSwatch] = useState(SWATCHES[0].id);
  // The Studio's two stage toggles. The simulated level starts off: the states alone
  // are calm enough to read; the level is there for whoever wants to see the reaction.
  const [sound, setSound] = useState(false);
  const [pointer, setPointer] = useState(true);
  const stage = useRef<HTMLElement>(null);
  const convo = useConversation(stage, hold);
  const turn = SCRIPT[convo.turn];
  const line = turn?.line?.slice(0, convo.shown) ?? "";

  return (
    <section className="lp-hero" aria-labelledby="lp-title">
      <div className="lp-hero-text">
        <h1 id="lp-title" className="lp-h1">
          {brand.copy.tagline}
        </h1>
        <p className="lp-lead">{brand.copy.heroLead}</p>
        <div className="lp-actions">
          <code className="lp-install">
            npm i {brand.packages.web}@{brand.npmTag}
          </code>
          <Link className="lp-button" href={brand.links.docs}>
            Read the docs
          </Link>
          <Link className="lp-link" href={brand.links.gallery}>
            See all {patternCount} patterns
          </Link>
          {brand.studioOpen ? (
            <Link className="lp-link" href={brand.links.studioPage}>
              Try the Studio
            </Link>
          ) : null}
        </div>
      </div>

      <figure ref={stage} className="lp-stage lp-hero-stage">
        <div className="lp-stage-top">
          <span className="lp-chip" data-live={convo.state !== "idle"}>
            <i aria-hidden="true" />
            {convo.state}
          </span>
          <span className="lp-stage-note">
            {hold ? "Held by you" : "Simulated conversation"}
            {sound ? ", with a simulated voice level" : ""}
          </span>
        </div>
        <LiveVisual
          pattern="glowing"
          state={convo.state}
          level={sound ? convo.level : 0}
          overrides={swatchOverrides(swatch)}
          pointer={pointer}
          className="lp-hero-visual"
          label={`A voice agent, ${convo.state}`}
        />
        <p className="lp-caption">
          {turn?.who ? <b>{turn.who}</b> : null}
          <span>{line || " "}</span>
        </p>
        <figcaption className="lp-stage-bar">
          <div className="seg" role="group" aria-label="Agent state">
            <button type="button" className="seg-btn" aria-pressed={hold === null} onClick={() => setHold(null)}>
              Auto
            </button>
            {VOICE_STATES.map((s) => (
              <button key={s} type="button" className="seg-btn" aria-pressed={hold === s} onClick={() => setHold(s)}>
                {s[0].toUpperCase() + s.slice(1)}
              </button>
            ))}
          </div>
          <div className="lp-toolbar">
            <button
              type="button"
              className="tb-btn tb-btn-icon"
              aria-pressed={pointer}
              aria-label="Pointer"
              title={pointer ? "Pointer on: point at the visual" : "Pointer off"}
              onClick={() => setPointer((v) => !v)}
            >
              <Icon name="pointer" />
            </button>
            <button
              type="button"
              className="tb-btn tb-btn-icon"
              aria-pressed={sound}
              aria-label="Simulated voice level"
              title={sound ? "Simulated voice level on" : "Simulated voice level off"}
              onClick={() => setSound((v) => !v)}
            >
              <Icon name="tone" />
            </button>
          </div>
          <div className="lp-swatches" role="radiogroup" aria-label="Colour">
            {SWATCHES.map((s) => (
              <button
                key={s.id}
                type="button"
                role="radio"
                aria-checked={swatch === s.id}
                aria-label={s.label}
                title={s.label}
                className="lp-swatch"
                style={{ background: s.css }}
                onClick={() => setSwatch(s.id)}
              />
            ))}
          </div>
        </figcaption>
      </figure>
    </section>
  );
}
