// Simulated conversations (docs/audio-pipeline.md, *Simulated conversations*): the
// engine's curves for a script of turns. Studio / demo tooling, so it lives in
// `@sinua/core/dev` (design note 33) and talks to the dev wasm.
import {
  conversation_at_json,
  conversation_samples_json,
  conversation_sample_json,
} from "../pkg-dev/sinua_core_inline.js";
import type { ConversationFrame } from "./index.js";

/**
 * A simulated conversation at `t` seconds: the agent state and a speech-like
 * level and `bands` bands, from a script of turns (docs/audio-pipeline.md,
 * *Simulated conversations*). Mirrors `core_engine::conversation_at`.
 */
export function conversationAt(script: string, t: number, bands = 16): ConversationFrame {
  return JSON.parse(conversation_at_json(script, t, bands)) as ConversationFrame;
}

/** The built-in sample conversations: `calendar`, `quick-answer`, `long-answer`, `barge-in`. */
export function conversationSampleNames(): string[] {
  return JSON.parse(conversation_samples_json()) as string[];
}

/** A built-in sample conversation's script (JSON text), or `null`. */
export function conversationSample(name: string): string | null {
  const s = conversation_sample_json(name);
  return s === "null" ? null : s;
}
