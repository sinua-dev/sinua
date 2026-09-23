import type { FxColor, FxContext, FxHsl, FxSpec, FxSpecResolved, OrbFrame } from '@sinua/react-native';

export function runSinuaCheckpoints(
  checkpoints: unknown,
  frameWithOverrides: (
    state: string,
    size: number,
    t: number,
    overrides: Record<string, number>,
  ) => Promise<OrbFrame | null>,
): Promise<string | null>;

export const FX_SPEC_EXAMPLE: FxSpec;
export const FX_SPEC_V11: FxSpec;

export function runFxSpecCheck(api: {
  resolveFxSpec: (spec: FxSpec, ctx?: FxContext) => Promise<FxSpecResolved>;
  frameFromFxSpec: (spec: FxSpec, elapsed: number, ctx?: FxContext) => Promise<OrbFrame | null>;
  fxColorToHsl: (color: FxColor) => Promise<FxHsl | null>;
  frameWithOverrides: (
    state: string,
    size: number,
    t: number,
    overrides: Record<string, number>,
  ) => Promise<OrbFrame | null>;
}): Promise<string | null>;
