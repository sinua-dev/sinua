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
  overrides: Record<string, number>;
}

/**
 * One solid colour, the same tone in both themes (`colorMode` fixed): the default
 * ink mode mirrors lightness on dark, which turns a hue pale there. 0.5 reads on the
 * dark stage; on light, LiveVisual's contrast correction takes the lightness down.
 */
export const TINT = { colorMix: 1, colorSaturation: 0.8, colorMode: 1, colorLightness: 0.5 };

const solid = (id: string, label: string, hue: number): Swatch => ({
  id,
  label,
  css: `hsl(${hue} 80% 60%)`,
  overrides: { ...TINT, colorHue: hue },
});

const ramp = (id: string, label: string, from: number, to: number): Swatch => ({
  id,
  label,
  css: `linear-gradient(135deg, hsl(${from} 80% 60%), hsl(${to % 360} 80% 60%))`,
  overrides: { gradientStrength: 1, gradientHue: from, gradientHue2: to, gradientSaturation: 0.8, gradientAngle: 135 },
});

export const SWATCHES: Swatch[] = [
  { id: "own", label: "The pattern's own colours", css: "conic-gradient(from 200deg, #6ee7b7, #a3e635, #22d3ee, #6ee7b7)", overrides: {} },
  solid("violet", "Violet", 265),
  solid("teal", "Teal", 172),
  solid("amber", "Amber", 38),
  solid("rose", "Rose", 345),
  ramp("dusk", "Violet to pink", 260, 330),
  ramp("sea", "Blue to green", 200, 150),
];

export function swatchOverrides(id: string): Record<string, number> {
  return SWATCHES.find((s) => s.id === id)?.overrides ?? {};
}
