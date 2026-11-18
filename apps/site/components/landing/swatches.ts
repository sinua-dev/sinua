/**
 * The colours the landing page offers: the pattern's own, four solid hues and
 * two ramps. Each is plain engine keys (`colorHue`, `gradientHue`, …), the same
 * keys a design passes as `overrides` or writes in an FX Spec file.
 */
export interface Swatch {
  id: string;
  label: string;
  /** How the swatch button itself is painted. */
  css: string;
  /** The dark-theme keys (and the one set the code snippets show). */
  overrides: Record<string, number>;
  /** Extra keys on the light theme: the deep tone that holds against white. */
  light?: Record<string, number>;
}

/** Light-theme tone for a fixed colour: darker, the same in both themes (`colorMode` fixed). */
const LIGHT_TINT = { colorMode: 1, colorMix: 1, colorLightness: -0.25 };

/**
 * One solid colour, the same tone in both themes (`colorMode` fixed): the default
 * ink mode mirrors lightness on dark, which turns a hue pale there. 0.5 reads on the
 * dark stage; on light, `tint` takes the lightness down to -0.25.
 */
export const TINT = { colorMix: 1, colorSaturation: 0.8, colorMode: 1, colorLightness: 0.4 };

/** A solid hue for either theme (the families, voice and look tiles). */
export const tint = (hue: number, theme: "light" | "dark") => ({ ...TINT, colorHue: hue, ...(theme === "light" ? LIGHT_TINT : {}) });

/** The Aurora ends as engine hues: teal and blue. */
export const AURORA_HUE = { teal: 165, blue: 217 };

/** The brand's default tint on an orb: the Aurora ramp (an override an app would pass; the engine's defaults are untouched). */
export const AURORA = { gradientStrength: 1, gradientHue: AURORA_HUE.teal, gradientHue2: AURORA_HUE.blue, gradientSaturation: 0.88 };

const solid = (id: string, label: string, hue: number): Swatch => ({
  id,
  label,
  css: `hsl(${hue} 80% 60%)`,
  overrides: { ...TINT, colorHue: hue },
  light: LIGHT_TINT,
});

const ramp = (id: string, label: string, from: number, to: number): Swatch => ({
  id,
  label,
  css: `linear-gradient(135deg, hsl(${from} 80% 60%), hsl(${to % 360} 80% 60%))`,
  overrides: { gradientStrength: 1, gradientHue: from, gradientHue2: to, gradientSaturation: 0.8, gradientAngle: 135 },
});

export const SWATCHES: Swatch[] = [
  { id: "aurora", label: "Aurora", css: "linear-gradient(135deg, #2de2b5, #3b82f6)", overrides: AURORA, light: LIGHT_TINT },
  { id: "own", label: "The pattern's own colours", css: "conic-gradient(from 200deg, #6ee7b7, #a3e635, #22d3ee, #6ee7b7)", overrides: {} },
  solid("violet", "Violet", 265),
  solid("teal", "Teal", 172),
  solid("amber", "Amber", 38),
  solid("rose", "Rose", 345),
  ramp("dusk", "Violet to pink", 260, 330),
  ramp("sea", "Blue to green", 200, 150),
];

export function swatchOverrides(id: string, theme: "light" | "dark" = "dark"): Record<string, number> {
  const s = SWATCHES.find((x) => x.id === id);
  return s ? { ...s.overrides, ...(theme === "light" ? s.light : {}) } : {};
}
