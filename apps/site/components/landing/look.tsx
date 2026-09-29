"use client";
/**
 * A taste of the Studio's workbench: a stage and a small inspector built from
 * the same controls (PgTabs, PgSlider, the Studio's material toggles and code
 * viewer). What you change here is plain engine keys, and the code under it is
 * the exporter's text for exactly those keys. Nothing is saved or exported.
 */
import { useMemo, useState } from "react";
import { Icon, PgSlider, PgTabs, Snippet } from "@sinua/design";
import { designTabs } from "@/lib/design-tabs";
import { LiveVisual } from "./live-visual";
import { SWATCHES } from "./swatches";

const PATTERNS = [
  { value: "glowing", label: "Glowing", object: "orb" },
  { value: "completing", label: "Completing", object: "ring" },
  { value: "metering", label: "Metering", object: "signal" },
] as const;

/** Each material's master key at a strength that reads clearly on its own. */
const MATERIALS: { id: string; label: string; keys: Record<string, number> }[] = [
  { id: "glow", label: "Glow", keys: { glowStrength: 0.7 } },
  { id: "noise", label: "Noise", keys: { noiseStrength: 0.6 } },
  { id: "pulse", label: "Pulse", keys: { pulseStrength: 0.7 } },
  { id: "liquid", label: "Liquid", keys: { liquidStrength: 1 } },
  { id: "particles", label: "Particles", keys: { particleStrength: 1 } },
  { id: "holographic", label: "Holographic", keys: { holoStrength: 1 } },
];

const SHOWCASE: Record<string, Record<string, number>> = { completing: { progress: 0.7 }, metering: { audioLevel: 0.75 } };

export function Look() {
  const [pattern, setPattern] = useState<(typeof PATTERNS)[number]["value"]>("glowing");
  const [swatch, setSwatch] = useState("violet");
  const [hue, setHue] = useState(265);
  const [on, setOn] = useState<Record<string, boolean>>({ glow: true });
  const [pointer, setPointer] = useState(true);
  const object = PATTERNS.find((p) => p.value === pattern)!.object;

  const colour = useMemo(() => {
    const s = SWATCHES.find((x) => x.id === swatch)!;
    // A solid swatch is a starting hue; the slider moves it.
    return "colorHue" in s.overrides ? { ...s.overrides, colorHue: hue } : s.overrides;
  }, [swatch, hue]);
  const overrides = useMemo(() => {
    const out: Record<string, number> = { ...colour };
    for (const m of MATERIALS) if (on[m.id]) Object.assign(out, m.keys);
    return out;
  }, [colour, on]);
  const tabs = useMemo(() => designTabs(object, pattern, overrides, ["typed-react", "swiftui", "compose", "typed-rn", "web"]), [object, pattern, overrides]);
  const solid = "colorHue" in (SWATCHES.find((x) => x.id === swatch)?.overrides ?? {});

  return (
    <section className="lp-section" aria-labelledby="lp-look">
      <header className="lp-section-head">
        <h2 id="lp-look" className="lp-h2">
          Your colours, your finish
        </h2>
        <p className="lp-lead">
          Every pattern takes a colour or a ramp, and wears any of the materials: glow, noise, pulse, liquid, particles, holographic. They are ordinary props,
          so the design lives in your code, or in one FX Spec file every platform reads the same way.
        </p>
      </header>

      <div className="lp-workbench">
        <div className="lp-stage lp-look-stage">
          <PointerToggle on={pointer} onChange={setPointer} />
          <LiveVisual key={pattern} pattern={pattern} overrides={{ ...SHOWCASE[pattern], ...overrides }} pointer={pointer} className="lp-look-visual" label={`${pattern} with your settings`} />
        </div>
        <div className="lp-inspector">
          <PgTabs label="Pattern" options={PATTERNS} value={pattern} onChange={setPattern} />
          <div className="pg-field">
            <span className="pg-label">
              <span>Colour</span>
            </span>
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
                  onClick={() => {
                    setSwatch(s.id);
                    if ("colorHue" in s.overrides) setHue(s.overrides.colorHue);
                  }}
                />
              ))}
            </div>
          </div>
          {solid ? <PgSlider label="Hue" value={hue} min={0} max={360} step={1} display={`${hue}°`} onChange={setHue} /> : null}
          <div className="pg-field">
            <span className="pg-label">
              <span>Materials</span>
            </span>
            <div className="lp-toggles">
              {MATERIALS.map((m) => (
                <button
                  key={m.id}
                  type="button"
                  className={"pg-tab" + (on[m.id] ? " pg-tab-active" : "")}
                  aria-pressed={Boolean(on[m.id])}
                  onClick={() => setOn({ ...on, [m.id]: !on[m.id] })}
                >
                  {m.label}
                </button>
              ))}
            </div>
          </div>
        </div>
        <div className="lp-workbench-code">
          <Snippet tabs={tabs} />
        </div>
      </div>
    </section>
  );
}

/** The Studio's pointer toggle, in the stage's corner. */
export function PointerToggle({ on, onChange }: { on: boolean; onChange: (on: boolean) => void }) {
  return (
    <button
      type="button"
      className="tb-btn tb-btn-icon lp-corner"
      aria-pressed={on}
      aria-label="Pointer"
      title={on ? "Pointer on: point at the visual" : "Pointer off"}
      onClick={() => onChange(!on)}
    >
      <Icon name="pointer" />
    </button>
  );
}
