// mount() under plain node: browser APIs stubbed, the canvas replaced by a
// context that records what gets drawn. Checks that the view draws exactly
// the frame the engine's own helpers produce for the same clock, plus the
// paint rules (reduced motion, DPR cap, pausing, a11y) and voice.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { frameFromFxSpec, frameWithOverrides, resolveFxSpec, resolvedOpts, voiceOverrides, voiceStateProfile, VoiceOverrides } from "@sinua/core";
import { mount, DPR_CAP } from "../dist/index.js";
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

test("the profile's speed rides the phase-continuous clock across a state change", () => {
  const { step } = env();
  const c = canvas();
  const idle = voiceStateProfile("working", "idle");
  const speaking = voiceStateProfile("working", "speaking");
  const preset = resolvedOpts("working", 64).speed;
  const fx = mount(c.el, { pattern: "working", state: "idle" });
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
