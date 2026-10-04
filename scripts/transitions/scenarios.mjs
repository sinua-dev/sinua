// The transition suite's conversations (design note 31, TS5): spec/transition-scenarios.json
// turned into the event and level stream a GPT-Live session sees at 30 Hz, run through the
// session core every platform shares (OpenAILiveSession), giving the states a view gets.
import { readFileSync } from "node:fs";

const { OpenAILiveSession } = await import(new URL("../../packages/voice/dist/openai.js", import.meta.url).href);
export const SCENARIOS = JSON.parse(readFileSync(new URL("../../spec/transition-scenarios.json", import.meta.url), "utf8")).scenarios;
const TICK = 1000 / 30;

/** A seeded random (mulberry32), so a scenario is the same on every run. */
function rng(seed) {
  let a = seed >>> 0;
  return () => {
    a = (a + 0x6d2b79f5) >>> 0;
    let t = a;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

/** The agent's level and the session events over time, in ms. */
function stream(sc) {
  const r = rng(7);
  const jitter = () => (sc.jitter ? (r() * 2 - 1) * sc.jitter : 0);
  const levels = []; // [fromMs, toMs, level]
  const events = []; // [ms, event]
  let t = 0;
  let d = 0;
  const segs = Array.from({ length: sc.repeat ?? 1 }, () => sc.segments).flat();
  for (const s of segs) {
    if (s.quiet != null) t += s.quiet * 1000;
    else if (s.user != null) {
      // Over the agent (a barge-in): the agent's voice stops shortly after the words start.
      if (s.over) levels.push([t, t + 150, 0.4]);
      for (let k = 0; k < s.user * 1000; k += 250) events.push([t + k + jitter(), { type: "session.input_transcript.delta", delta: "w" }]);
      t += s.user * 1000;
    } else if (s.think != null) {
      const id = `d${d++}`;
      events.push([t + jitter(), { type: "session.delegation.created", delegation: { id } }]);
      events.push([t + s.think * 1000 + jitter(), { type: "response.event", delegation_id: id, event: { type: "response.completed" } }]);
      t += s.think * 1000;
    } else if (s.say != null) {
      const pauses = s.pauses === "natural" ? natural(s.say, r) : (s.pauses ?? []);
      let at = t;
      for (const [sec, ms] of pauses) {
        levels.push([at, t + sec * 1000, 0.4]);
        at = t + sec * 1000 + ms;
      }
      levels.push([at, t + s.say * 1000, 0.4]);
      t += s.say * 1000;
    }
  }
  events.sort((a, b) => a[0] - b[0]);
  return { levels, events, end: t };
}

/** Pauses as people speak: every 2-4 s, 300-900 ms. */
function natural(seconds, r) {
  const out = [];
  for (let s = 2 + r() * 2; s < seconds - 1; s += 2 + r() * 2) out.push([s, 300 + r() * 600]);
  return out;
}

/** The states a view gets: [[seconds, state], ...] (the first at 0), and the agent's answers. */
export function states(sc) {
  const { levels, events, end } = stream(sc);
  const s = new OpenAILiveSession();
  const out = [];
  s.onState = (st) => out.push([now / 1000, st]);
  let now = 0;
  s.connecting();
  s.handle(JSON.stringify({ type: "session.started", session: { id: "live" } }), 0);
  let e = 0;
  for (now = 0; now <= end + 2000; now += TICK) {
    while (e < events.length && events[e][0] <= now) s.handle(JSON.stringify(events[e++][1]), now);
    const level = levels.some(([a, b]) => now >= a && now < b) ? 0.4 : 0;
    s.tick(level, now);
  }
  // Collapse to changes; the first state at 0.
  const changes = [];
  for (const [t, st] of out) if (!changes.length || changes.at(-1)[1] !== st) changes.push([t, st]);
  if (!changes.length || changes[0][0] > 0) changes.unshift([0, changes[0]?.[1] === "listening" ? "listening" : "idle"]);
  changes[0][0] = 0;
  // Answers: the agent's say segments (each should be one speaking stretch).
  const answers = Array.from({ length: sc.repeat ?? 1 }, () => sc.segments).flat().filter((x) => x.say != null).length;
  return { changes, answers, seconds: end / 1000 + 2 };
}
