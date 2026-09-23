"use client";
/**
 * The rest of the landing page. The visuals are the same `mount` path as the
 * hero, the code is the Studio's own exporter (@sinua/snippets), and every
 * string comes from lib/brand.ts.
 */
import { useState } from "react";
import Link from "next/link";
import { parameterCatalog } from "@sinua/core";
import { buildSnippets, buildTypedSnippets, toTypedProps } from "@sinua/snippets";
import { brand } from "@/lib/brand";
import { LiveVisual } from "./live-visual";

/**
 * One pattern per use case, with the values that make it read at a glance:
 * a ring needs progress, a waveform needs a level. Static values, not fake
 * behaviour -- the motion is the pattern's own.
 */
const USE_CASES = [
  { pattern: "speaking", overrides: { audioLevel: 0.62, audioStrength: 0.45 } },
  { pattern: "tracking", overrides: { progress0: 0.72, progress1: 0.48, progress2: 0.3 } },
  { pattern: "waveform", overrides: { audioLevel: 0.7, amplitude: 0.34 } },
] as const;

export function UseCases() {
  return (
    <section className="lp-section lp-block" aria-labelledby="lp-usecases">
      <h2 id="lp-usecases" className="lp-h2">
        What it draws
      </h2>
      <p className="lp-lead">{brand.copy.useCasesLead}</p>
      <ul className="lp-cases">
        {brand.copy.useCases.map((c, i) => (
          <li key={c.title} className="lp-case">
            <LiveVisual pattern={USE_CASES[i].pattern} overrides={{ ...USE_CASES[i].overrides }} maxFps={30} className="lp-case-visual" label={c.title} />
            <div>
              <h3 className="lp-h3">{c.title}</h3>
              <p className="lp-case-line">{c.line}</p>
            </div>
          </li>
        ))}
      </ul>
    </section>
  );
}

/**
 * The design the snippets describe: the hero's pattern at its defaults. The
 * typed component comes first (that's what we tell people to write); the
 * generic view follows for the platforms that read better with it.
 */
const DESIGN = { object: "orb", pattern: "glowing" };
const TYPED = buildTypedSnippets({ design: toTypedProps(DESIGN.object, DESIGN.pattern, {}, parameterCatalog()), size: 64 });
const VIEW = buildSnippets({ state: DESIGN.pattern, size: 64, overrides: {}, specFile: `${DESIGN.object}-${DESIGN.pattern}.fxspec.json` });
const SNIPPETS = {
  code: [
    ...TYPED.map((t) => ({ ...t, id: `typed-${t.id}` })),
    ...VIEW.code.filter((t) => t.id !== "react" && t.id !== "rn").map((t) => ({ ...t, label: t.label })),
  ],
};

export function Frameworks() {
  const [id, setId] = useState(SNIPPETS.code[0].id);
  const tab = SNIPPETS.code.find((t) => t.id === id) ?? SNIPPETS.code[0];
  return (
    <section className="lp-section lp-block" aria-labelledby="lp-frameworks">
      <h2 id="lp-frameworks" className="lp-h2">
        One design, every front end
      </h2>
      <p className="lp-lead">{brand.copy.frameworksLead}</p>
      <div className="lp-tabs" role="tablist" aria-label="Platform">
        {SNIPPETS.code.map((t) => (
          <button key={t.id} type="button" role="tab" aria-selected={t.id === tab.id} className="lp-tab" data-on={t.id === tab.id} onClick={() => setId(t.id)}>
            {t.label}
          </button>
        ))}
      </div>
      <pre className="lp-code" role="tabpanel" aria-label={`${tab.label} code`}>
        {tab.code.split("\n\n// Or use the file")[0]}
      </pre>
    </section>
  );
}

export function Pricing() {
  return (
    <section className="lp-section lp-block" aria-labelledby="lp-pricing">
      <h2 id="lp-pricing" className="lp-h2">
        What costs money
      </h2>
      <p className="lp-lead">{brand.copy.pricingLead}</p>
      <div className="lp-plans">
        <div className="lp-plan">
          <h3 className="lp-h3">{brand.copy.pricingFree}</h3>
          <p className="lp-case-line">{brand.copy.packagesPitch}</p>
          <code className="lp-install">npm i {brand.packages.web}</code>
        </div>
        <div className="lp-plan lp-plan-paid">
          <h3 className="lp-h3">{brand.copy.pricingPaid}</h3>
          <p className="lp-case-line">{brand.copy.studioPitch}</p>
          <Link className="lp-link" href={brand.links.docs}>
            How it fits your app
          </Link>
        </div>
      </div>
    </section>
  );
}
