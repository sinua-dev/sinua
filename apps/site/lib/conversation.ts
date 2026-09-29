"use client";
/**
 * A simulated voice conversation for the landing page: the engine's own
 * simulator (`conversationAt`, the same one behind `SimulatedVoiceSource`) playing
 * its "calendar" sample. The states come in order with the words being said and a
 * speech-like level, and no microphone or network is ever opened.
 *
 * One clock serves every section that shows it (the hero, the voice section), so
 * they move in step and only one loop runs. It runs only while one of them is on
 * screen; off screen, nothing re-renders.
 */
import { useEffect, useRef, useState, type RefObject } from "react";
import calendar from "../../../spec/conversations/calendar.json";
import type { VoiceState } from "./voice-state";

export interface Turn {
  state: VoiceState;
  /** Seconds this turn lasts. */
  seconds: number;
  who?: "You" | "Agent";
  line?: string;
}

/** The sample's turns, for the timeline and the captions (the engine reads the same file). */
export const SCRIPT: Turn[] = calendar.turns.map((t) => ({
  state: t.state as VoiceState,
  seconds: t.seconds,
  line: "line" in t ? t.line : undefined,
  // Who is talking: the user while listening; the agent's words otherwise.
  who: "voice" in t && t.voice === "user" ? "You" : "line" in t ? "Agent" : undefined,
}));

const TOTAL = SCRIPT.reduce((s, t) => s + t.seconds, 0);
const STARTS = SCRIPT.map((_, i) => SCRIPT.slice(0, i).reduce((s, t) => s + t.seconds, 0));
const JSON_TEXT = JSON.stringify(calendar);

type Engine = typeof import("@sinua/core");
let engine: Engine | null = null;
let loading: Promise<void> | null = null;
function loadEngine() {
  loading ??= import("@sinua/core").then((m) => {
    engine = m;
  });
  return loading;
}

export interface ConversationFrame {
  /** Index into SCRIPT. */
  turn: number;
  state: VoiceState;
  level: number;
  /** 0..1 through the current turn. */
  progress: number;
  /** How many characters of the current line are on screen (it is "typed" as it is said). */
  shown: number;
  /** The engine's 16 frequency bands, eased like the level: a spectrum that moves like speech. */
  bands: number[];
}

const QUIET: number[] = new Array<number>(16).fill(0);
const FIRST: ConversationFrame = { turn: 0, state: "idle", level: 0, progress: 0, shown: 0, bands: QUIET };

// ---- the one clock ----------------------------------------------------------

interface Subscriber {
  hold: VoiceState | null;
  /** The eased level and bands this subscriber shows. */
  level: number;
  bands: number[];
  visible: boolean;
  set: (f: ConversationFrame) => void;
}

const subscribers = new Set<Subscriber>();
let raf = 0;
let prev = 0;
let lastEmit = 0;
let scriptClock = 0;

function reducedMotion(): boolean {
  try {
    return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
  } catch {
    return false;
  }
}

/**
 * The engine's frame at a script time. Its level moves like speech, syllable by
 * syllable; the page eases it (`smooth`) so the hero stays calm when its sound is on.
 */
function engineAt(t: number) {
  return engine ? engine.conversationAt(JSON_TEXT, t, 16) : null;
}

function frameFor(hold: VoiceState | null, now: number): ConversationFrame {
  const reduced = reducedMotion();
  if (hold) {
    // A held state keeps its sound moving: loop through that turn of the script.
    const turn = Math.max(0, SCRIPT.findIndex((t) => t.state === hold));
    const t = STARTS[turn] + ((now / 1000) % SCRIPT[turn].seconds);
    const f = engineAt(t);
    const level = f ? f.level : 0;
    return { turn, state: hold, level: reduced ? 0.5 * Number(level > 0) : level, progress: 1, shown: SCRIPT[turn]?.line?.length ?? 0, bands: reduced || !f ? QUIET : f.bands };
  }
  const f = engineAt(scriptClock);
  if (!f) return FIRST;
  const line = SCRIPT[f.turn]?.line ?? "";
  return {
    turn: f.turn,
    state: f.state as VoiceState,
    level: reduced ? 0.5 * Number(f.level > 0) : f.level,
    progress: f.progress,
    shown: reduced ? line.length : Math.min(line.length, f.shown),
    bands: reduced ? QUIET : f.bands,
  };
}

/**
 * A one-pole ease toward the engine's level, per subscriber, at the emit rate. Rate 4
 * (measured on the calendar sample): the frame-to-frame change drops from 0.068 to
 * 0.016 while the level still peaks near 0.45, so the hero stays calm with its sound on.
 */
function smooth(prev: number, next: number, dt: number, rate = 4) {
  return prev + (next - prev) * Math.min(1, dt * rate);
}

function tick(now: number) {
  raf = 0;
  const active = [...subscribers].filter((s) => s.visible);
  if (!active.length) return; // nothing on screen: stop until one comes back
  const dt = Math.min(0.1, (now - prev) / 1000);
  prev = now;
  // The script moves while anyone is watching it run (a held hero doesn't stop the voice section).
  if (active.some((s) => !s.hold)) scriptClock = (scriptClock + dt) % TOTAL;
  // The engine draws at its own rate; the page only feeds it inputs, 30 times a second.
  if (now - lastEmit >= 33) {
    lastEmit = now;
    for (const s of active) {
      const f = frameFor(s.hold, now);
      s.level = smooth(s.level, f.level, 0.033);
      // The bands ease faster than the level: they carry the voice's texture, which rate 4 flattened.
      s.bands = f.bands.map((b, i) => smooth(s.bands[i] ?? 0, b, 0.033, 12));
      s.set({ ...f, level: f.level === 0 ? 0 : s.level, bands: f.level === 0 ? QUIET : s.bands });
    }
  }
  raf = requestAnimationFrame(tick);
}

function wake() {
  if (!engine) {
    void loadEngine().then(wake);
    return;
  }
  if (raf) return;
  prev = performance.now();
  raf = requestAnimationFrame(tick);
}

/**
 * The conversation as `where` sees it. `hold` pins one state (the visitor picked
 * it); the level keeps moving so a held listening or speaking state still reads as
 * sound. Nothing updates while `where` is off screen.
 */
export function useConversation(where: RefObject<Element | null>, hold: VoiceState | null = null): ConversationFrame {
  const [frame, setFrame] = useState<ConversationFrame>(FIRST);
  const sub = useRef<Subscriber | null>(null);
  useEffect(() => {
    if (sub.current) sub.current.hold = hold;
  }, [hold]);

  useEffect(() => {
    const me: Subscriber = { hold, level: 0, bands: QUIET, visible: false, set: setFrame };
    sub.current = me;
    subscribers.add(me);
    const el = where.current;
    const io =
      el && typeof IntersectionObserver !== "undefined"
        ? new IntersectionObserver((entries) => {
            me.visible = entries[entries.length - 1]?.isIntersecting ?? true;
            if (me.visible) wake();
          })
        : null;
    if (io && el) io.observe(el);
    else {
      me.visible = true;
      wake();
    }
    return () => {
      io?.disconnect();
      subscribers.delete(me);
      sub.current = null;
    };
    // `hold` is read through the subscriber, not a dependency: changing it must not resubscribe.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [where]);

  return frame;
}
