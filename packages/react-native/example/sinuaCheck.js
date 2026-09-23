// Checks a few spec/sinua-golden.json cases (via sinuaCheckpoints.json)
// through whichever `frameWithOverrides` it's handed. Plain JS on purpose:
// the RN app runs it through the native bridge, and
// packages/core/test/sinua-checkpoints.test.mjs runs the *same function*
// against the wasm build, so the logic is proven before it ships to a
// device. Returns null on success, or a precise failure message.
//
// A cross-platform lock, not a correctness proof -- see docs/testing.md.

const near = (a, b, tol) => Math.abs(a - b) < tol;
const dotRow = (d) => [d.x, d.y, d.z, d.r, d.white, d.a, d.saturation, d.hue];
const polyStyle = (p) => [p.white, p.a, p.w, p.saturation, p.hue];

function cmp(label, got, exp, tol) {
  for (let k = 0; k < exp.length; k++) {
    if (!near(got[k], exp[k], tol)) return `${label}[${k}]: expected ${exp[k]}, got ${got[k]}`;
  }
  return null;
}

function checkPolyline(key, name, p, exp, tol) {
  const pts = p.points;
  return (
    cmp(`${key}: ${name}.style`, polyStyle(p), exp.style, tol) ||
    cmp(`${key}: ${name}.first`, [pts[0].x, pts[0].y], exp.first, tol) ||
    cmp(`${key}: ${name}.last`, [pts[pts.length - 1].x, pts[pts.length - 1].y], exp.last, tol)
  );
}

export async function runSinuaCheckpoints(checkpoints, frameWithOverrides) {
  const tol = checkpoints.tolerance;
  for (const c of checkpoints.cases) {
    const f = await frameWithOverrides(c.state, c.size, c.t, c.overrides);
    if (!f) return `${c.key}: frameWithOverrides resolved to null`;
    if (f.dots.length !== c.dotCount) return `${c.key}: dots=${f.dots.length} (expected ${c.dotCount})`;
    if (f.lines.length !== c.lineCount) return `${c.key}: lines=${f.lines.length} (expected ${c.lineCount})`;
    if (c.colorMode !== undefined && f.colorMode !== c.colorMode) {
      return `${c.key}: colorMode=${f.colorMode} (expected ${c.colorMode})`;
    }
    if (f.polylines.length !== c.polylineCount) {
      return `${c.key}: polylines=${f.polylines.length} (expected ${c.polylineCount})`;
    }
    const err =
      (c.firstDot && cmp(`${c.key}: firstDot`, dotRow(f.dots[0]), c.firstDot, tol)) ||
      (c.lastDot && cmp(`${c.key}: lastDot`, dotRow(f.dots[f.dots.length - 1]), c.lastDot, tol)) ||
      (c.firstPolyline && checkPolyline(c.key, 'firstPolyline', f.polylines[0], c.firstPolyline, tol)) ||
      (c.lastPolyline &&
        checkPolyline(c.key, 'lastPolyline', f.polylines[f.polylines.length - 1], c.lastPolyline, tol));
    if (err) return err;
  }
  return null;
}

// FX Spec through the bridge (docs/fx-spec.md). The spec is inlined (the
// app can't read spec/examples/); packages/core/test/sinua-checkpoints
// .test.mjs fails if it drifts from spec/examples/beacon-radar-glow.fxspec.json.
// `elapsed = 0` so the comparison needs no preset speed (RN exposes none).
export const FX_SPEC_EXAMPLE = {
  $schema: '../fx-spec-1.schema.json',
  fxSpec: '1.8',
  name: 'Glowing radar',
  object: 'beacon',
  pattern: 'scanning',
  size: 64,
  color: { value: { colorSpace: 'srgb', components: [0.2, 0.8, 0.4], hex: '#33cc66' }, mix: 0.8 },
  materials: { glow: { strength: 0.6, radius: 3 } },
  params: { blipCount: 4 },
};

// A minimal lifecycle spec (the keys FX Spec 1.1 added): one entry with an input binding.
// Keyed `talking`, not an agent state, so 1.8's voice-state profile stays out of
// the numbers this check compares (it would set audioStrength itself).
export const FX_SPEC_V11 = {
  fxSpec: '1.8',
  object: 'orb',
  pattern: 'breathing',
  states: { talking: { pattern: 'speaking', bindings: { audioLevel: { input: 'agentVolume' } } } },
};

function sameFrame(a, b) {
  const rows = (f) => [
    ...f.dots.map(dotRow),
    ...f.lines.map((l) => [l.x1, l.y1, l.x2, l.y2, l.white, l.a, l.w, l.saturation, l.hue]),
    ...f.polylines.map((p) => [...polyStyle(p), ...p.points.flatMap((q) => [q.x, q.y])]),
  ];
  const [x, y] = [rows(a), rows(b)];
  if (a.colorMode !== b.colorMode || x.length !== y.length) return false;
  return x.every((r, i) => r.length === y[i].length && r.every((v, k) => v === y[i][k]));
}

/** `api` = { resolveFxSpec(spec, ctx?), frameFromFxSpec(spec, elapsed, ctx?), frameWithOverrides, fxColorToHsl }, all async. */
export async function runFxSpecCheck(api) {
  const r = await api.resolveFxSpec(FX_SPEC_EXAMPLE);
  if (!r.ok) return `fxSpec: example has errors ${JSON.stringify(r.diagnostics)}`;
  if (r.diagnostics.length) return `fxSpec: unexpected warnings ${JSON.stringify(r.diagnostics)}`;
  const got = await api.frameFromFxSpec(FX_SPEC_EXAMPLE, 0);
  const want = await api.frameWithOverrides(r.state, r.size, 0, r.overrides);
  if (!got || !want || !sameFrame(got, want)) return 'fxSpec: frameFromFxSpec != frameWithOverrides(resolved)';
  if (got.dots.length === 0 || r.overrides.glowStrength !== 0.6) return 'fxSpec: glow/colour not resolved';
  const bad = await api.resolveFxSpec({ ...FX_SPEC_EXAMPLE, params: { blipCont: 4 } });
  if (bad.ok || !bad.diagnostics.some((d) => d.path === '/params/blipCont' && d.message.includes('`blipCount`'))) {
    return `fxSpec: typo not diagnosed ${JSON.stringify(bad.diagnostics)}`;
  }
  // v1.1: a state entry + a binding through the bridge (ctx = { state, inputs }).
  const ctx = { state: 'talking', inputs: { agentVolume: 0.5 } };
  const v = await api.resolveFxSpec(FX_SPEC_V11, ctx);
  if (!v.ok || v.stateKey !== 'talking' || v.state !== 'speaking') return `fxSpec 1.1: state ${JSON.stringify(v)}`;
  if (v.overrides.audioLevel !== 0.5 || v.overrides.audioStrength !== 0.18) {
    return `fxSpec 1.1: binding ${JSON.stringify(v.overrides)}`;
  }
  const vf = await api.frameFromFxSpec(FX_SPEC_V11, 0, ctx);
  const vw = await api.frameWithOverrides(v.state, v.size, 0, v.overrides);
  if (!vf || !vw || !sameFrame(vf, vw)) return 'fxSpec 1.1: frameFromFxSpec(ctx) != frameWithOverrides(resolved)';
  const idle = await api.resolveFxSpec(FX_SPEC_V11, { state: 'talking' });
  if (idle.inactiveBindings.join() !== 'audioLevel') return `fxSpec 1.1: inactive ${JSON.stringify(idle)}`;
  const c = await api.fxColorToHsl('#ff00ff');
  if (!c || c.hex !== '#ff00ff' || Math.abs(c.h - 300) > 1e-9) return `fxSpec: fxColorToHsl ${JSON.stringify(c)}`;
  return null;
}
