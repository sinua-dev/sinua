// Drives a pattern through state changes the way every view does (design note 31): the
// real StateTransition (the weight clock), the engine, and the phase integrated at the
// clock's speed. Each 60 Hz frame is rasterized; the suite measures how far the picture
// moves from frame to frame.
import { raster, diff } from "./raster.mjs";

export const core = await import(new URL("../../packages/core/dist/index.js", import.meta.url).href);
// The catalog is a dev API (@sinua/core/dev); the frames stay on the default build.
const { parameterCatalog } = await import(new URL("../../packages/core/dist-dev/dev-entry.js", import.meta.url).href);
export const ORBS = core.ORB_STATES ?? "working searching solving listening connecting weaving composing breathing shaping glowing drifting speaking confirming initializing calibrating progressing muted silhouette signaling waveform scrolling metering playing completing loading tracking stepping measuring talking notifying".split(" ");
export const CHARACTERS = "bean beep buzzy chirp cuppa hum wisp".split(" ");
// A new orb or character pattern must join the lists above (the silhouette once didn't).
for (const [object, list] of [["orb", ORBS], ["character", CHARACTERS]]) {
  const missing = parameterCatalog().objects.find((o) => o.id === object).patterns.map((p) => p.id).filter((id) => !list.includes(id));
  if (missing.length) throw new Error(`transitions: ${object} patterns missing from scripts/transitions/drive.mjs: ${missing.join(", ")}`);
}
export const VOICE = ["initializing", "idle", "listening", "thinking", "speaking"];
export const SIZE = 64;
export const DT = 1 / 60;
const N = 96;

const sides = new Map();
/** A pattern's side for a voice state, as a plain view builds it (profile under no app overrides). */
export function side(pattern, state) {
  const key = `${pattern}/${state}`;
  if (!sides.has(key)) {
    const p = core.voiceStateProfile(pattern, state);
    sides.set(key, { state: pattern, speed: (core.resolvedOpts(pattern, SIZE)?.speed ?? 1) * (p?.speed ?? 1), overrides: { ...(p?.overrides ?? {}) } });
  }
  return sides.get(key);
}

/**
 * Runs `events` ([seconds, state], the first is the start) for `total` seconds from session
 * time `t0`. Returns per frame the picture's change `d` and whether a change happened in the
 * last `window` seconds, plus each change's `look` (how different the two states' pictures
 * are at that phase: what a perfect transition spreads out). `gaps` ([seconds, dt]) injects
 * long frames (a hitch, the background).
 */
export function run(pattern, events, { t0 = 10, total, window = 1.0, gaps = [] } = {}) {
  const tr = new core.StateTransition();
  let cur = events[0][1];
  let phase = t0 * side(pattern, cur).speed;
  let ev = 1;
  let prev = null;
  let lastChange = -Infinity;
  let after = false;
  const frames = [];
  const looks = [];
  for (let i = 0, t = 0; t <= total + 1e-9; i++, t = i * DT) {
    let dt = DT;
    const gap = gaps.find(([at]) => Math.abs(at - t) < DT / 2);
    if (gap) dt = gap[1];
    while (ev < events.length && t >= events[ev][0] - 1e-9) {
      const next = events[ev][1];
      const pair = core.fxSpecTransition("{}", cur, next);
      const look = diff(raster(core.frameWithOverrides(pattern, SIZE, phase, side(pattern, cur).overrides), SIZE, N), raster(core.frameWithOverrides(pattern, SIZE, phase, side(pattern, next).overrides), SIZE, N));
      looks.push({ look, duration: pair.duration });
      tr.start(pair.duration, pair.curve);
      cur = next;
      ev++;
      lastChange = t;
    }
    if (i > 0) tr.advance(dt);
    const to = side(pattern, cur);
    phase += Math.min(dt, 0.1) * tr.speed(to, SIZE);
    const f = tr.frames(to, SIZE, phase, {});
    const g = new Float32Array(N * N);
    raster(f.frame, SIZE, N, g, f.blend);
    if (f.previous) raster(f.previous, SIZE, N, g, 1 - f.blend);
    // The first frame after a gap over 1 s lands on the target by rule: labelled, not judged.
    const landed = after;
    after = dt > 1;
    if (prev) frames.push({ t, d: diff(prev, g), inTr: t - lastChange < window + 1e-9, landed });
    prev = g;
  }
  return { frames, looks };
}

/**
 * peak: the worst transition frame over what a smooth one would show (the steady motion's
 * largest frame plus each change's look spread over its own duration, the clock's peak
 * rate being ~1.6× its mean). path: how far the picture travels during transitions over the steady motion
 * plus the looks' difference (boiling travels far for a small difference). 1 = ideal.
 */
export function score({ frames, looks }) {
  const steady = frames.filter((f) => !f.inTr && !f.landed).map((f) => f.d);
  const smax = Math.max(5e-4, ...steady);
  const smean = steady.reduce((a, b) => a + b, 0) / Math.max(1, steady.length);
  const rate = Math.max(0, ...looks.map((l) => l.look / Math.max(0.1, l.duration)));
  const trs = frames.filter((f) => f.inTr && !f.landed);
  const peak = Math.max(0, ...trs.map((f) => f.d));
  const path = trs.reduce((a, f) => a + f.d, 0);
  return {
    peak: peak / (smax + 1.6 * rate * DT),
    path: path / (looks.reduce((a, l) => a + l.look, 0) + smean * trs.length + 1e-3 * Math.max(1, looks.length)),
  };
}
