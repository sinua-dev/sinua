// mount() under plain node: browser APIs stubbed, the canvas replaced by a
// context that records what gets drawn. Checks that the view draws exactly
// the frame the engine's own helpers produce for the same clock, plus the
// paint rules (reduced motion, DPR cap, pausing, a11y) and voice.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { frameFromFxSpec, frameWithOverrides, resolveFxSpec, resolvedOpts, transitionMix, voiceOverrides, voiceStateProfile, VoiceOverrides } from "@sinua/core";
import { mount, viewLayout, DPR_CAP } from "../dist/index.js";
import { env, canvas } from "./dom.mjs";

const specText = readFileSync(new URL("../../../spec/examples/voice-assistant.fxspec.json", import.meta.url), "utf8");


const arcs = (calls) => calls.filter((c) => c[0] === "arc").map((c) => c.slice(1));
const dotsOf = (frame) => frame.dots.map((d) => [d.x, d.y, d.r]);

test("spec input draws exactly frameFromFxSpec(spec, elapsed)", () => {
  const { step } = env();
  const c = canvas();
  const fx = mount(c.el, { spec: specText, theme: "light" });
  step(90);
  assert.ok(fx.elapsed > 1.4 && fx.elapsed < 1.6, `elapsed ${fx.elapsed}`);
  assert.deepEqual(arcs(c.calls), dotsOf(frameFromFxSpec(specText, fx.elapsed)));
  assert.ok(arcs(c.calls).length > 0);
  fx.destroy();
});

test("plain state input: t = elapsed * presetSpeed * speed", async () => {
  const { resolvedOpts } = await import("@sinua/core");
  const { step } = env();
  const c = canvas();
  const fx = mount(c.el, { pattern: "listening", size: 64, speed: 1.5 });
  step(30);
  const t = fx.elapsed * resolvedOpts("listening", 64).speed * 1.5;
  assert.deepEqual(arcs(c.calls), dotsOf(frameWithOverrides("listening", 64, t, {})));
  fx.destroy();
});

test("reduced motion: static frame at t = 0.6, no loop without a voice", () => {
  const { step, rafs } = env({ reduce: true });
  const c = canvas();
  const fx = mount(c.el, { pattern: "listening" });
  assert.deepEqual(arcs(c.calls), dotsOf(frameWithOverrides("listening", 64, 0.6, {})));
  step(5);
  assert.equal(rafs.filter(Boolean).length, 0, "no animation loop");
  fx.destroy();
});

test("DPR is capped at 2 for the backing store", () => {
  env({ dpr: 3 });
  const c = canvas(100);
  const fx = mount(c.el, { pattern: "listening" });
  assert.equal(DPR_CAP, 2);
  assert.equal(c.el.width, 200);
  assert.equal(c.el.height, 200);
  fx.destroy();
});

test("pauses when the tab is hidden, on pause() and on paused: true", () => {
  const h = env({ hidden: true });
  const c = canvas();
  const fx = mount(c.el, { pattern: "listening" });
  h.step(10);
  assert.equal(fx.elapsed, 0, "hidden tab: clock stopped");
  globalThis.document.hidden = false;
  fx.resume();
  h.step(10);
  const e = fx.elapsed;
  assert.ok(e > 0);
  fx.pause();
  h.step(10);
  assert.equal(fx.elapsed, e, "pause() freezes");
  fx.resume();
  fx.update({ paused: true });
  h.step(10);
  assert.equal(fx.elapsed, e, "paused option freezes");
  fx.destroy();
});

test("an update to a view nobody can see draws nothing; it draws once it is visible again", () => {
  // Hidden tab: an app feeding a level 60 times a second into a view in a
  // background tab must not paint 60 frames a second.
  const h = env({ hidden: true });
  const c = canvas();
  const fx = mount(c.el, { pattern: "speaking" });
  const before = c.clears;
  for (let i = 0; i < 20; i++) fx.update({ overrides: { audioLevel: i / 20 } });
  h.step(5);
  assert.equal(c.clears - before, 0, "hidden tab: 20 updates, no frame drawn");
  globalThis.document.hidden = false;
  fx.resume(); // what the visibilitychange listener does
  assert.ok(c.clears > before, "visible again: the latest state is drawn");
  fx.destroy();

  // Scrolled off screen: the same, through the IntersectionObserver.
  let report = null;
  globalThis.IntersectionObserver = class {
    constructor(cb) { report = (visible) => cb([{ isIntersecting: visible }]); }
    observe() {}
    disconnect() {}
  };
  try {
    env();
    const d = canvas();
    const fy = mount(d.el, { pattern: "speaking" });
    report(false);
    const off = d.clears;
    for (let i = 0; i < 20; i++) fy.update({ overrides: { audioLevel: i / 20 } });
    assert.equal(d.clears - off, 0, "off screen: 20 updates, no frame drawn");
    report(true);
    assert.equal(d.clears - off, 1, "back on screen: one frame, right away");
    fy.destroy();
  } finally {
    delete globalThis.IntersectionObserver;
  }
});

test("a paused view that is visible still draws its still frame on update", () => {
  env();
  const c = canvas();
  const fx = mount(c.el, { pattern: "speaking", paused: true });
  const before = c.clears;
  fx.update({ overrides: { audioLevel: 0.5 } });
  assert.equal(c.clears - before, 1, "paused but visible: not blank, one still frame");
  fx.destroy();
});

// Dots compared to 1e-9: the pointer's eased strength is summed frame by frame, and
// that sum's last bits depend on the clock's float dt.
const close = (a, b) => a.length === b.length && a.every((p, i) => p.every((v, k) => Math.abs(v - b[i][k]) < 1e-9));

test("pointer: the dots near the pointer are pushed, with the Studio's radius and strength", () => {
  const { step } = env();
  const c = canvas(100);
  const fx = mount(c.el, { pattern: "working", pointer: true });
  // (60, 40) in a 100px box is (38.4, 25.6) in a 64-unit engine square.
  c.fire("pointermove", { clientX: 60, clientY: 40 });
  step(200); // long enough for the eased strength to settle at 1
  const t = fx.elapsed * resolvedOpts("working", 64).speed;
  const keys = { pointerX: 38.4, pointerY: 25.6, pointerRadius: 64 * 0.35, pointerStrength: 64 * 0.12 };
  assert.ok(close(arcs(c.calls), dotsOf(frameWithOverrides("working", 64, t, keys))), "the drawn frame is the engine's frame with the pointer keys");
  assert.ok(!close(arcs(c.calls), dotsOf(frameWithOverrides("working", 64, t, {}))), "and it differs from the frame without them");

  c.fire("pointerleave");
  step(200); // the strength eases back to 0
  const t2 = fx.elapsed * resolvedOpts("working", 64).speed;
  assert.deepEqual(arcs(c.calls), dotsOf(frameWithOverrides("working", 64, t2, {})), "after the pointer leaves: the plain frame again");

  assert.ok(c.listening > 0);
  fx.destroy();
  assert.equal(c.listening, 0, "destroy removes the pointer listeners");
});

test("pointer: off by default, under reduced motion, and after update({ pointer: false })", () => {
  env();
  const off = canvas(100);
  const a = mount(off.el, { pattern: "working" });
  assert.equal(off.listening, 0, "no pointer option: no listeners at all");
  a.destroy();

  const { step } = env({ reduce: true });
  const c = canvas(100);
  const fx = mount(c.el, { pattern: "working", pointer: true });
  c.fire("pointermove", { clientX: 60, clientY: 40 });
  step(200);
  fx.update({ overrides: {} }); // draws the reduced-motion pose now
  assert.deepEqual(arcs(c.calls), dotsOf(frameWithOverrides("working", 64, 0.6, {})), "reduced motion: no scatter");
  fx.update({ pointer: false });
  assert.equal(c.listening, 0, "update({ pointer: false }) unbinds");
  fx.destroy();
});

test("voice keys are merged into the frame", () => {
  const { step } = env();
  const c = canvas();
  const voice = new VoiceOverrides({ bandEaseRate: Infinity, levelEaseRate: Infinity });
  voice.push({ level: 0.9, bands: Array.from({ length: 16 }, (_, i) => (i % 3) / 2) });
  voice.setState("speaking");
  const fx = mount(c.el, { pattern: "speaking", voice });
  step(20);
  assert.equal(fx.voice, voice, "a passed VoiceOverrides is used as-is");
  // A bound source also supplies the lifecycle state, so the built-in voice-state
  // profile applies under the voice's live keys (docs/fx-view.md, *Voice states*).
  const profile = voiceStateProfile("speaking", "speaking");
  const t = fx.elapsed * resolvedOpts("speaking", 64).speed * profile.speed;
  const map = voiceOverrides(voice.metrics, "speaking", { bandEaseRate: Infinity, levelEaseRate: Infinity });
  assert.deepEqual(arcs(c.calls), dotsOf(frameWithOverrides("speaking", 64, t, { ...profile.overrides, ...map })));
  assert.notDeepEqual(arcs(c.calls), dotsOf(frameWithOverrides("speaking", 64, t, profile.overrides)), "voice changes the drawn frame");
  fx.destroy();
});

test("a voice's AgentState picks the spec's v1.1 state (after the cross-fade)", () => {
  const { step } = env();
  const c = canvas();
  const voice = new VoiceOverrides();
  const fx = mount(c.el, { spec: specText, voice, crossFade: 0 });
  voice.setState("speaking");
  step(40);
  const r = resolveFxSpec(specText, { state: "speaking" });
  assert.equal(r.stateKey, "speaking", "fixture: the spec has a speaking state");
  // What the view must draw: the speaking resolution + the voice's runtime keys spread last.
  const t = fx.elapsed * resolvedOpts(r.state, r.size).speed * r.speed;
  const want = frameWithOverrides(r.state, r.size, t, { ...r.overrides, ...voiceOverrides({ level: 0, bands: [] }, "speaking") });
  assert.deepEqual(arcs(c.calls), dotsOf(want));
  assert.notDeepEqual(dotsOf(want), dotsOf(frameFromFxSpec(specText, fx.elapsed)), "and it differs from the base design");
  fx.destroy();
});

test("invalid spec: onError with diagnostics, nothing drawn", () => {
  env();
  const c = canvas();
  let diags = null;
  const fx = mount(c.el, { spec: '{"fxSpec":"1.8","object":"orb","pattern":"nope"}', onError: (d) => (diags = d) });
  assert.ok(Array.isArray(diags) && diags.length > 0);
  assert.equal(arcs(c.calls).length, 0);
  fx.destroy();
});

test("accessibility: role img + label from the spec name; empty label = decorative", () => {
  env();
  const c = canvas();
  const fx = mount(c.el, { spec: specText });
  assert.equal(c.attrs.role, "img");
  assert.equal(c.attrs["aria-label"], JSON.parse(specText).name);
  fx.update({ label: "" });
  assert.equal(c.attrs["aria-hidden"], "true");
  assert.equal(c.attrs.role, undefined);
  fx.destroy();
});

test("maxFps skips draws but the clock keeps wall time", () => {
  const { step } = env();
  const c = canvas();
  let draws = 0;
  const fx = mount(c.el, { pattern: "listening", maxFps: 30, onFrame: () => draws++ });
  draws = 0;
  step(120); // 2 s at 60 Hz
  assert.ok(draws >= 59 && draws <= 61, `draws ${draws}`);
  assert.ok(Math.abs(fx.elapsed - 2) < 0.05, `elapsed ${fx.elapsed}`);
  fx.destroy();
});

test("lowPower: 30 fps and glow off (the block-less default)", () => {
  const { step } = env();
  const c = canvas();
  let draws = 0;
  const fx = mount(c.el, { pattern: "composing", overrides: { glowStrength: 1 }, lowPower: true, onFrame: () => draws++ });
  draws = 0;
  step(60);
  assert.ok(draws >= 29 && draws <= 31, `draws ${draws}`);
  const t = fx.elapsed * resolvedOpts("composing", 64).speed;
  assert.deepEqual(arcs(c.calls), dotsOf(frameWithOverrides("composing", 64, t, { glowStrength: 0 })));
  assert.notDeepEqual(arcs(c.calls), dotsOf(frameWithOverrides("composing", 64, t, { glowStrength: 1 })), "glow was actually on before");
  fx.update({ lowPower: false });
  draws = 0;
  step(60);
  assert.ok(draws >= 59, `back to display rate: ${draws}`);
  fx.destroy();
});

test("onFrame reports dt, compute and paint times", () => {
  const { step } = env();
  const c = canvas();
  const stats = [];
  const fx = mount(c.el, { pattern: "listening", onFrame: (s) => stats.push(s) });
  step(5);
  const last = stats.at(-1);
  assert.ok(Math.abs(last.dtMs - 1000 / 60) < 0.01, `dt ${last.dtMs}`);
  assert.ok(last.computeMs >= 0 && last.paintMs >= 0);
  fx.destroy();
});

test("FX Spec 1.2: the spec's performance block caps and sheds under low power", () => {
  const { step } = env();
  const c = canvas();
  const spec = JSON.stringify({
    fxSpec: "1.8", object: "orb", pattern: "composing",
    materials: { glow: { strength: 1 } },
    performance: { maxFps: 50, lowPower: { maxFps: 20, disable: ["glow"] } },
  });
  let draws = 0;
  const fx = mount(c.el, { spec, onFrame: () => draws++ });
  draws = 0;
  step(60);
  assert.ok(draws >= 49 && draws <= 51, `spec maxFps 50 on 60 Hz: ${draws}`);
  fx.update({ lowPower: true });
  draws = 0;
  step(60);
  assert.ok(draws >= 19 && draws <= 21, `lowPower maxFps 20: ${draws}`);
  const r = resolveFxSpec(spec, { lowPower: true });
  assert.deepEqual(r.disabledMaterials, ["glow"]);
  const t = fx.elapsed * resolvedOpts(r.state, r.size).speed * r.speed;
  assert.deepEqual(arcs(c.calls), dotsOf(frameWithOverrides(r.state, r.size, t, r.overrides)), "drawn = the resolver's shed frame");
  fx.destroy();
});


test("lowPower (no spec block) also sheds particles: the drawn frame equals particleStrength 0", () => {
  const { step } = env();
  const c = canvas();
  const fx = mount(c.el, { pattern: "glowing", overrides: { particleStrength: 1 }, lowPower: true });
  step(30);
  const t = fx.elapsed * resolvedOpts("glowing", 64).speed;
  const shed = dotsOf(frameWithOverrides("glowing", 64, t, { particleStrength: 0, glowStrength: 0 }));
  assert.deepEqual(arcs(c.calls), shed);
  assert.ok(dotsOf(frameWithOverrides("glowing", 64, t, { particleStrength: 1, glowStrength: 0 })).length > shed.length, "particles were really on");
  fx.destroy();
});

test("FX Spec 1.7 labels: pattern, state (lifecycle); the old ones still work", () => {
  const { step } = env();
  const warnings = [];
  const warn = console.warn;
  console.warn = (m) => warnings.push(String(m));
  try {
    const a = canvas();
    const b = canvas();
    const fa = mount(a.el, { pattern: "listening" });
    const fb = mount(b.el, { state: "listening" }); // deprecated: no spec, so state = pattern
    step(20);
    assert.ok(arcs(a.calls).length > 0);
    assert.deepEqual(arcs(b.calls), arcs(a.calls));
    assert.equal(warnings.filter((w) => w.includes("deprecated")).length, 1, "warned once");
    fa.destroy();
    fb.destroy();

    // With a spec, `state` is the lifecycle key -- the same as the deprecated `specState`.
    const s1 = canvas();
    const s2 = canvas();
    const f1 = mount(s1.el, { spec: specText, state: "speaking", crossFade: 0 });
    const f2 = mount(s2.el, { spec: specText, specState: "speaking", crossFade: 0 });
    step(20);
    const r = resolveFxSpec(specText, { state: "speaking" });
    const t = f1.elapsed * resolvedOpts(r.state, r.size).speed * r.speed;
    assert.deepEqual(arcs(s1.calls), dotsOf(frameWithOverrides(r.state, r.size, t, r.overrides)));
    assert.deepEqual(arcs(s2.calls), arcs(s1.calls));
    f1.destroy();
    f2.destroy();
  } finally {
    console.warn = warn;
  }
});

test("changing the lifecycle state doesn't rebuild the spec player; changing the pattern does", () => {
  const { step } = env();
  const c = canvas();
  const fx = mount(c.el, { spec: specText, state: "listening", crossFade: 10 });
  step(5);
  fx.update({ state: "speaking" }); // a cross-fade starts
  step(5);
  const mid = arcs(c.calls);
  const r = resolveFxSpec(specText, { state: "speaking" });
  const t = fx.elapsed * resolvedOpts(r.state, r.size).speed * r.speed;
  assert.notDeepEqual(mid, dotsOf(frameWithOverrides(r.state, r.size, t, r.overrides)), "still cross-fading, not cut");
  fx.destroy();
  const p = canvas();
  const fp = mount(p.el, { pattern: "listening" });
  fp.update({ pattern: "speaking" });
  step(5);
  const t2 = fp.elapsed * resolvedOpts("speaking", 64).speed;
  assert.deepEqual(arcs(p.calls), dotsOf(frameWithOverrides("speaking", 64, t2, {})));
  fp.destroy();
});

/**
 * How far two frames of the same pattern are apart (engine units), dot by dot.
 * A different dot count (e.g. glow adds arcs) counts as "completely different".
 */
const frameDistance = (a, b) => (a.length !== b.length ? Infinity : Math.max(...a.map(([x, y], i) => Math.hypot(x - b[i][0], y - b[i][1]))));
/** The arcs of exactly one drawn frame. */
const oneFrame = (c, step) => {
  c.calls.length = 0;
  step(1);
  return arcs(c.calls);
};
const frameAt = (t) => dotsOf(frameWithOverrides("listening", 64, t, {}));

test("a speed change continues the phase instead of jumping the pose", () => {
  const { step } = env();
  const c = canvas();
  const fx = mount(c.el, { pattern: "listening", speed: 1 });
  step(119); // two seconds in: the old formula would rescale all of it at once
  oneFrame(c, step);
  const elapsedBefore = fx.elapsed;
  const preset = resolvedOpts("listening", 64).speed;
  fx.update({ speed: 3 });
  const after = oneFrame(c, step);
  assert.ok(
    frameDistance(after, frameAt(elapsedBefore * preset + (1 / 60) * preset * 3)) < 1e-6,
    "one step at the new speed, continuing from the phase it had",
  );
  assert.ok(
    frameDistance(after, frameAt(fx.elapsed * preset * 3)) > 1,
    "not the old `elapsed * speed` pose, which would jump two seconds of phase",
  );
  fx.destroy();
});

test("speed 0 holds the pose, and a later speed keeps accumulating from it", () => {
  const { step } = env();
  const c = canvas();
  const fx = mount(c.el, { pattern: "listening", speed: 1 });
  step(59);
  const held = oneFrame(c, step);
  const heldElapsed = fx.elapsed;
  const preset = resolvedOpts("listening", 64).speed;
  fx.update({ speed: 0 });
  step(60);
  assert.ok(frameDistance(oneFrame(c, step), held) < 1e-9, "frozen at the phase it had");
  fx.update({ speed: 1 });
  step(29);
  const resumed = oneFrame(c, step);
  assert.ok(frameDistance(resumed, held) > 1, "moving again");
  assert.ok(
    frameDistance(resumed, frameAt(heldElapsed * preset + (30 / 60) * preset)) < 1e-6,
    "resumed from the held phase, not from elapsed",
  );
  fx.destroy();
});

test("a spec state with its own speed doesn't jump the phase either (families' repro)", () => {
  const { step } = env();
  const c = canvas();
  // Two states that differ only in speed: any jump must come from the clock, not the design.
  const spec = JSON.stringify({
    fxSpec: "1.8",
    object: "orb",
    pattern: "listening",
    speed: 0.6,
    states: { slow: { speed: 0.6 }, fast: { speed: 1.15 } },
  });
  const fx = mount(c.el, { spec, state: "slow", crossFade: 0 });
  step(30);
  oneFrame(c, step);
  const elapsedBefore = fx.elapsed;
  const fast = resolveFxSpec(spec, { state: "fast" });
  const preset = resolvedOpts(fast.state, fast.size).speed;
  const phaseBefore = elapsedBefore * preset * 0.6;
  fx.update({ state: "fast" });
  const after = oneFrame(c, step);
  const continued = dotsOf(frameWithOverrides(fast.state, fast.size, phaseBefore + (1 / 60) * preset * 1.15, fast.overrides));
  const jumped = dotsOf(frameWithOverrides(fast.state, fast.size, fx.elapsed * preset * 1.15, fast.overrides));
  assert.ok(frameDistance(after, continued) < 1e-6, "one step at the new state's speed, from the phase it had");
  assert.ok(frameDistance(after, jumped) > 1, "not the old rescaled-elapsed pose");
  fx.destroy();
});

// --- Voice states without a spec: pattern + state (FX Spec 1.8 profiles) ---

test("pattern + state applies the voice-state profile under the app's own overrides", () => {
  const { step } = env();
  const c = canvas();
  const profile = voiceStateProfile("working", "listening");
  assert.ok(profile && profile.overrides.audioStrength < 0, "fixture: listening draws inward");
  const fx = mount(c.el, { pattern: "working", state: "listening", overrides: { glowStrength: 0.9 } });
  step(20);
  const drawn = oneFrame(c, step);
  const t = fx.elapsed * resolvedOpts("working", 64).speed * profile.speed;
  const expected = dotsOf(frameWithOverrides("working", 64, t, { ...profile.overrides, glowStrength: 0.9 }));
  assert.ok(frameDistance(drawn, expected) < 1e-9, "profile first, the app's overrides on top");
  // The app's own value really wins over the profile's.
  const profileGlow = dotsOf(frameWithOverrides("working", 64, t, profile.overrides));
  assert.ok(frameDistance(drawn, profileGlow) > 1e-6, "not the profile's glow");
  fx.destroy();
});

test("a state outside the voice names changes nothing, and neither does no state", () => {
  const { step } = env();
  const plain = canvas();
  const named = canvas();
  assert.equal(voiceStateProfile("working", "goalReached"), null, "fixture: an app's own state name");
  const a = mount(plain.el, { pattern: "working" });
  const b = mount(named.el, { pattern: "working", state: "goalReached" });
  step(20);
  assert.deepEqual(arcs(named.calls), arcs(plain.calls), "an app's own state is left alone");
  a.destroy();
  b.destroy();
});

test("audioInput names which app input drives audioLevel", () => {
  const { step } = env();
  const c = canvas();
  const profile = voiceStateProfile("working", "listening");
  assert.equal(profile.audioInput, "micLevel", "fixture: listening follows the user's level");
  const fx = mount(c.el, { pattern: "working", state: "listening", inputs: { micLevel: 0.8 } });
  step(20);
  const drawn = oneFrame(c, step);
  const t = fx.elapsed * resolvedOpts("working", 64).speed * profile.speed;
  const expected = dotsOf(frameWithOverrides("working", 64, t, { ...profile.overrides, audioLevel: 0.8 }));
  assert.ok(frameDistance(drawn, expected) < 1e-9, "the named input drives audioLevel");
  const without = dotsOf(frameWithOverrides("working", 64, t, profile.overrides));
  assert.ok(frameDistance(drawn, without) > 1e-6, "and it changes the frame");
  fx.destroy();
});

test("crossFade: 0 cuts: the profile's speed rides the phase-continuous clock across a state change", () => {
  const { step } = env();
  const c = canvas();
  const idle = voiceStateProfile("working", "idle");
  const speaking = voiceStateProfile("working", "speaking");
  const preset = resolvedOpts("working", 64).speed;
  const fx = mount(c.el, { pattern: "working", state: "idle", crossFade: 0 });
  step(59);
  oneFrame(c, step);
  const phaseBefore = fx.elapsed * preset * idle.speed;
  fx.update({ state: "speaking" });
  const after = oneFrame(c, step);
  const continued = dotsOf(
    frameWithOverrides("working", 64, phaseBefore + (1 / 60) * preset * speaking.speed, speaking.overrides),
  );
  assert.ok(frameDistance(after, continued) < 1e-9, "continues from the idle phase at the speaking speed");
  fx.destroy();
});

test("a plain view's state change flows: parameters and speed interpolate over 0.6 s, then land exactly", () => {
  const { step } = env();
  const c = canvas();
  const idle = voiceStateProfile("working", "idle");
  const speaking = voiceStateProfile("working", "speaking");
  const preset = resolvedOpts("working", 64).speed;
  const fx = mount(c.el, { pattern: "working", state: "idle" });
  step(59);
  oneFrame(c, step);
  let phase = fx.elapsed * preset * idle.speed;
  fx.update({ state: "speaking" });
  const from = { state: "working", speed: preset * idle.speed, overrides: idle.overrides };
  const to = { state: "working", speed: preset * speaking.speed, overrides: speaking.overrides };
  // One frame in: the mix at 1/60 of 0.6 s, the phase advanced at the mixed speed.
  const first = oneFrame(c, step);
  const mix = transitionMix(from, to, 64, 1 / 60 / 0.6, "easeInOut");
  assert.equal(mix.technique, "params");
  phase += (1 / 60) * mix.speed;
  const expected = Object.keys(mix.structuralTo).length && mix.swap > 0
    ? null
    : dotsOf(frameWithOverrides("working", 64, phase, mix.overrides));
  if (expected) assert.ok(frameDistance(first, expected) < 1e-9, "the first frame is the mix, not speaking");
  assert.ok(frameDistance(first, dotsOf(frameWithOverrides("working", 64, phase, speaking.overrides))) > 1e-6, "not a jump");
  // Frame by frame the phase advances at the mixed speed; after 0.6 s it is speaking exactly.
  step(40);
  for (let k = 2; k <= 41; k++) phase += (1 / 60) * (transitionMix(from, to, 64, Math.min(1, k / 60 / 0.6), "easeInOut").speed);
  const last = oneFrame(c, step);
  phase += (1 / 60) * to.speed;
  assert.ok(frameDistance(last, dotsOf(frameWithOverrides("working", 64, phase, speaking.overrides))) < 1e-6, "lands on speaking");
  fx.destroy();
});

test("a 1.7 spec with a state renders exactly as before: no profile is applied", () => {
  const { step } = env();
  const c = canvas();
  const spec = JSON.stringify({
    fxSpec: "1.8",
    object: "orb",
    pattern: "working",
    states: { listening: { materials: { glow: { strength: 0.42 } } } },
  });
  const fx = mount(c.el, { spec, state: "listening", crossFade: 0 });
  step(20);
  const drawn = oneFrame(c, step);
  const r = resolveFxSpec(spec, { state: "listening" });
  const t = fx.elapsed * resolvedOpts(r.state, r.size).speed * r.speed;
  assert.ok(
    frameDistance(drawn, dotsOf(frameWithOverrides(r.state, r.size, t, r.overrides))) < 1e-9,
    "the resolver's own result, with no voice-state profile mixed in",
  );
  fx.destroy();
});

test("two views on one raw source both follow it, each with its own tracker, and show its mute", async () => {
  const { SharedVoiceSource } = await import("@sinua/core");
  const { step } = env();
  const a = canvas();
  const b = canvas();
  // One callback of each kind, like every real source: a second direct bind would steal it.
  const src = { cbs: {}, onMetrics(cb) { this.cbs.m = cb; }, onStateChange(cb) { this.cbs.s = cb; }, async connect() {}, disconnect() {}, setMuted() {} };
  const orb = mount(a.el, { pattern: "working", voice: src });
  const signal = mount(b.el, { pattern: "waveform", voice: src });
  assert.notEqual(orb.voice, signal.voice, "each view keeps its family's tracker");
  src.cbs.s("listening");
  src.cbs.m({ level: 0.6, bands: Array(16).fill(0.3) });
  assert.equal(orb.voice.state, "listening");
  assert.equal(signal.voice.state, "listening");
  assert.equal(orb.voice.metrics.level, 0.6);
  assert.equal(signal.voice.metrics.level, 0.6);
  SharedVoiceSource.of(src).setMuted(true);
  assert.equal(orb.voice.overrides(0.016).muted, 1);
  assert.equal(signal.voice.overrides(0.016).muted, 1);
  step(2);
  orb.destroy();
  src.cbs.m({ level: 0.2, bands: Array(16).fill(0.1) });
  assert.equal(orb.voice.metrics.level, 0.6, "a destroyed view is unsubscribed");
  assert.equal(signal.voice.metrics.level, 0.2);
  signal.destroy();
});

// --- Accessibility (docs/fx-view.md, *Accessibility*) and the 1.9 rules glue ---

/** A canvas with a parent (for the live region), a fake document, and a clock the announcer reads. */
function a11yEnv(t) {
  const e = env();
  const c = canvas();
  const kids = [];
  c.el.parentNode = { insertBefore: (node) => kids.push(node) };
  globalThis.document.createElement = () => {
    const attrs = {};
    return { style: {}, textContent: "", setAttribute: (k, v) => (attrs[k] = v), attrs, remove() { kids.splice(kids.indexOf(this), 1); } };
  };
  let now = 0;
  const realNow = performance.now.bind(performance);
  performance.now = () => now * 1000;
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const advance = (s) => {
    now += s;
    t.mock.timers.tick(s * 1000);
  };
  t.after(() => (performance.now = realNow));
  const said = () => kids.map((k) => k.textContent);
  return { ...e, c, kids, advance, said };
}

/** One callback of each kind, like every real source. */
const stateSource = () => ({ cbs: {}, onMetrics(cb) { this.cbs.m = cb; }, onStateChange(cb) { this.cbs.s = cb; }, async connect() {}, disconnect() {} });

test("the accessible name follows the voice's state, and a held change is spoken once", async (t) => {
  const { c, advance, said, kids } = a11yEnv(t);
  const src = stateSource();
  const fx = mount(c.el, { pattern: "glowing", voice: src });
  assert.equal(c.attrs["aria-label"], "glowing");
  src.cbs.s("listening");
  await Promise.resolve();
  assert.equal(c.attrs["aria-label"], "glowing, listening");
  assert.equal(kids.length, 0, "nothing spoken before the 1 s hold");
  advance(1.1);
  assert.deepEqual(said(), ["glowing, listening"]);
  assert.equal(kids[0].attrs["aria-live"], "polite");
  // A flip back and forth under a second says nothing more.
  src.cbs.s("speaking");
  await Promise.resolve();
  advance(0.4);
  src.cbs.s("listening");
  await Promise.resolve();
  advance(5);
  assert.deepEqual(said(), ["glowing, listening"]);
  fx.destroy();
  assert.equal(kids.length, 0, "the live region leaves with the view");
});

test("labels win over the spec's words; announce: false keeps quiet but still renames", async (t) => {
  const { c, advance, said } = a11yEnv(t);
  const spec = JSON.stringify({
    fxSpec: "1.9", object: "orb", pattern: "glowing", name: "Coach",
    states: { listening: {}, speaking: {} },
    accessibility: { states: { listening: "Coach is listening", speaking: "Coach is speaking" } },
  });
  const src = stateSource();
  const fx = mount(c.el, { spec, voice: src, labels: { speaking: "Koç konuşuyor" } });
  assert.equal(c.attrs["aria-label"], "Coach");
  src.cbs.s("listening");
  await Promise.resolve();
  assert.equal(c.attrs["aria-label"], "Coach is listening");
  src.cbs.s("speaking");
  await Promise.resolve();
  assert.equal(c.attrs["aria-label"], "Koç konuşuyor");
  advance(1.1);
  assert.deepEqual(said(), ["Koç konuşuyor"]);
  fx.update({ announce: false });
  src.cbs.s("listening");
  await Promise.resolve();
  advance(5);
  assert.equal(c.attrs["aria-label"], "Coach is listening");
  assert.deepEqual(said(), ["Koç konuşuyor"], "quiet after announce: false");
  fx.destroy();
});

test("rules derive the state from inputs with hysteresis; a voice or rules: false turns them off", async (t) => {
  const { c } = a11yEnv(t);
  const spec = JSON.stringify({
    fxSpec: "1.9", object: "orb", pattern: "glowing", name: "Heart",
    states: { intense: {} },
    rules: [{ when: { input: "hr", gt: 150 }, state: "intense", hysteresis: 5 }],
    accessibility: { states: { intense: "Heart rate high" } },
  });
  const fx = mount(c.el, { spec, inputs: { hr: 140 } });
  const label = () => c.attrs["aria-label"];
  assert.equal(label(), "Heart");
  fx.update({ inputs: { hr: 151 } });
  assert.equal(label(), "Heart rate high", "enter above 150");
  fx.update({ inputs: { hr: 148 } });
  assert.equal(label(), "Heart rate high", "held by the hysteresis");
  fx.update({ inputs: { hr: 145 } });
  assert.equal(label(), "Heart", "left at 145");
  fx.update({ inputs: { hr: 160 }, rules: false });
  assert.equal(label(), "Heart", "rules: false");
  fx.update({ rules: true });
  assert.equal(label(), "Heart rate high");
  const src = stateSource();
  fx.update({ voice: src });
  src.cbs.s("listening");
  await Promise.resolve();
  assert.equal(label(), "Heart, listening", "a bound voice drives the state instead");
  fx.destroy();
});

// Roadmap 9/10 (sinua-b1): box layout, one rAF for many views, the small-view cap.

/** The x/y extent of every path point drawn (engine units: `scale` isn't applied here). */
const pathExtent = (calls) => {
  const pts = calls.filter((c) => c[0] === "moveTo" || c[0] === "lineTo");
  return { maxX: Math.max(...pts.map((c) => c[1])), maxY: Math.max(...pts.map((c) => c[2])), n: pts.length };
};

test("a box-layout pattern fills a wide box: the box ratio goes in as aspect, nothing is centred", () => {
  const { step } = env();
  const c = canvas();
  c.el.clientWidth = 400;
  c.el.clientHeight = 100;
  // Grey ink (saturation 0): the stub has no getTransform for per-vertex colour; the
  // geometry is what's under test.
  const fx = mount(c.el, { pattern: "framing", overrides: { idleOpacity: 0.5, saturation: 0 }, reducedMotion: "never" });
  step(2);
  const { maxX, maxY, n } = pathExtent(c.calls);
  assert.ok(n > 0, "the rim was stroked");
  // aspect 4: engine space is 256 x 64; the rim's right side sits near x = 256.
  assert.ok(maxX > 240 && maxX <= 256, `right edge at ${maxX}`);
  assert.ok(maxY > 56 && maxY <= 64, `bottom edge at ${maxY}`);
  assert.ok(!c.calls.some((k) => k[0] === "translate"), "no centring translate");
  assert.deepEqual(c.calls.find((k) => k[0] === "scale"), ["scale", 100 / 64], "scaled by the box height");
  fx.destroy();
});

test("a square pattern in the same wide box is still a centred square", () => {
  const { step } = env();
  const c = canvas();
  c.el.clientWidth = 400;
  c.el.clientHeight = 100;
  const fx = mount(c.el, { pattern: "completing", overrides: { progress: 0.5 } });
  step(2);
  assert.deepEqual(c.calls.find((k) => k[0] === "translate"), ["translate", 150, 0]);
  assert.ok(pathExtent(c.calls).maxX <= 64);
  fx.destroy();
});

test("many views share one requestAnimationFrame per display frame", () => {
  const { rafs, step } = env();
  const views = Array.from({ length: 12 }, () => mount(canvas().el, { pattern: "working" }));
  let draws = 0;
  views.forEach((v) => v.update({ onFrame: () => draws++ }));
  step(1);
  assert.equal(rafs.filter(Boolean).length, 1, "one rAF queued for twelve views");
  draws = 0;
  step(10);
  assert.equal(draws, 120, "and every view still draws every frame");
  views[0].destroy();
  step(1);
  assert.equal(rafs.filter(Boolean).length, 1);
  views.slice(1).forEach((v) => v.destroy());
  assert.equal(rafs.filter(Boolean).length, 0, "the last view out cancels the frame");
});

test("a small view defaults to 30 fps; maxFps or the spec's performance block wins", () => {
  const { step } = env();
  const count = (opts, css = 32) => {
    const c = canvas(css);
    let draws = 0;
    const fx = mount(c.el, { pattern: "listening", ...opts, onFrame: () => draws++ });
    draws = 0;
    step(60);
    fx.destroy();
    return draws;
  };
  const small = count({});
  assert.ok(small >= 29 && small <= 31, `32 px: ${small}`);
  assert.ok(count({}, 64) >= 59, "64 px: display rate");
  assert.ok(count({ maxFps: 0 }) >= 59, "maxFps 0: the app asked for display rate");
  assert.ok(count({ maxFps: 60 }) >= 59, "maxFps 60 wins over the small default");
  const spec = JSON.stringify({ fxSpec: "1.8", object: "orb", pattern: "listening", performance: { maxFps: 60 } });
  assert.ok(count({ spec, pattern: undefined }) >= 59, "the spec's maxFps wins too");
});

test("a view that shrinks below the small size picks up the 30 fps default", () => {
  const { step } = env();
  const c = canvas(100);
  const listeners = [];
  const RO = globalThis.ResizeObserver;
  globalThis.ResizeObserver = class { constructor(cb) { listeners.push(cb); } observe() {} disconnect() {} };
  try {
    let draws = 0;
    const fx = mount(c.el, { pattern: "listening", onFrame: () => draws++ });
    draws = 0;
    step(60);
    assert.ok(draws >= 59, `100 px: ${draws}`);
    listeners[0]([{ contentRect: { width: 32, height: 32 } }]);
    draws = 0;
    step(60);
    assert.ok(draws >= 29 && draws <= 32, `32 px after resize: ${draws}`);
    fx.destroy();
  } finally {
    globalThis.ResizeObserver = RO;
  }
});

// --- One-shot effects (docs/fx-view.md, *One-shot effects*) ---

test("trigger plays an effect for its duration, speaks its words, then the view is as before", async (t) => {
  const { c, step, advance, said } = a11yEnv(t);
  const fx = mount(c.el, { pattern: "completing", label: "Goal", labels: { "effect:celebrate": "Hedef tamam" } });
  const strokes = () => c.calls.filter((x) => x[0] === "lineTo").length;
  step(1);
  const before = strokes();
  fx.trigger("success");
  assert.deepEqual(said(), ["Done"], "spoken at once, outside the state rate limit");
  step(1);
  assert.ok(strokes() > before, "the ring and the tick are drawn");
  advance(1.0); // past 0.9 s
  step(1);
  assert.equal(strokes(), before, "gone after its duration");
  fx.trigger("celebrate");
  assert.deepEqual(said(), ["Hedef tamam"], "labels['effect:<name>'] win");
  fx.update({ announce: false });
  fx.trigger("error");
  assert.deepEqual(said(), ["Hedef tamam"], "announce: false keeps quiet");
  fx.trigger("confetti"); // unknown: nothing
  fx.destroy();
});

test("under reduced motion an effect still draws (its reduced variant) and then stops", async (t) => {
  const { c, step, advance } = a11yEnv(t);
  globalThis.window.matchMedia = (q) => ({ matches: q.includes("reduce"), addEventListener() {}, removeEventListener() {} });
  const fx = mount(c.el, { pattern: "completing", reducedMotion: "always" });
  const strokes = () => c.calls.filter((x) => x[0] === "lineTo").length;
  const before = strokes();
  fx.trigger("success");
  step(3, 40);
  assert.ok(strokes() > before, "the tick shows in place");
  advance(1.0);
  step(3, 40);
  assert.equal(strokes(), before);
  fx.destroy();
});

test("viewLayout: box for the box-layout patterns (plain or spec), square otherwise", () => {
  assert.equal(viewLayout({ pattern: "framing" }), "box");
  assert.equal(viewLayout({ pattern: "playing" }), "box");
  assert.equal(viewLayout({ pattern: "completing" }), "square");
  assert.equal(viewLayout({ spec: { fxSpec: "1.9", object: "edge", pattern: "framing" } }), "box");
  assert.equal(viewLayout({ spec: specText }), "square");
  assert.equal(viewLayout({ pattern: "no-such-pattern" }), "square", "invalid input is square");
});
