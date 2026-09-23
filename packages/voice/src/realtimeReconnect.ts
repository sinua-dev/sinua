// DOM-free reconnect rules for `OpenAIRealtimeVoiceSource`, kept apart so they can
// be tested under plain node.
//
// OpenAI Realtime has no session resumption (researched 2026-09-18, see
// docs/audio-pipeline.md, *Reconnect*): a dropped call can only be replaced
// by a *new* session, with the client re-submitting prior turns via
// `conversation.item.create`. The policy numbers and the fatal-code list
// follow LiveKit's shipped OpenAI Realtime client
// (`livekit-agents/livekit/agents/llm/_realtime/openai.py`, `types.py`
// `APIConnectOptions`): 3 retries, the first almost immediate.

export interface ReconnectPolicy {
  /** Attempts after a drop before giving up. Default 3 (LiveKit's `max_retry`). */
  maxAttempts?: number;
  /** Base of the exponential delay from the second attempt on. Default 1000 ms. */
  baseDelayMs?: number;
  /** Delay cap. Default 8000 ms. */
  maxDelayMs?: number;
}

export const DEFAULT_RECONNECT_ATTEMPTS = 3;
const FIRST_DELAY_MS = 100; // LiveKit's `_interval_for_retry(0)`: a blip usually clears at once
const DEFAULT_BASE_DELAY_MS = 1000;
const DEFAULT_MAX_DELAY_MS = 8000;
const JITTER = 0.2;

/**
 * Delay before reconnect attempt `attempt` (1-based): 100 ms, then
 * `base * 2^(attempt-2)` capped at `maxDelayMs`, each ±20 % jitter so many
 * clients dropped by the same outage don't retry in lockstep.
 */
export function reconnectDelayMs(attempt: number, policy: ReconnectPolicy = {}, random: () => number = Math.random): number {
  const base = policy.baseDelayMs ?? DEFAULT_BASE_DELAY_MS;
  const cap = policy.maxDelayMs ?? DEFAULT_MAX_DELAY_MS;
  const nominal = attempt <= 1 ? FIRST_DELAY_MS : Math.min(cap, base * 2 ** (attempt - 2));
  const jitter = 1 + JITTER * (2 * random() - 1);
  return Math.max(0, Math.round(nominal * jitter));
}

/**
 * Error codes that can never succeed on retry -- LiveKit's
 * `_FATAL_ERROR_CODES`, verbatim: the connection comes up, every
 * generation fails, the server closes again, and a retry loop spins forever.
 */
export const FATAL_REALTIME_ERROR_CODES: ReadonlySet<string> = new Set([
  "insufficient_quota",
  "invalid_api_key",
  "account_deactivated",
  "billing_hard_limit_reached",
]);

export function isFatalRealtimeError(code: unknown): boolean {
  return typeof code === "string" && FATAL_REALTIME_ERROR_CODES.has(code);
}

/**
 * Whether a failed HTTP call (`/v1/realtime/calls`, `client_secrets`) is
 * worth retrying: timeouts, rate limits and server errors are; any other
 * 4xx (bad or expired key, bad request) will fail the same way again.
 */
export function isRetryableHttpStatus(status: number): boolean {
  return status === 408 || status === 425 || status === 429 || status >= 500;
}

/** A thrown connect failure the reconnect loop must not retry. */
export class FatalConnectError extends Error {
  readonly fatal = true;
}

export function isFatalConnectError(err: unknown): boolean {
  return err instanceof FatalConnectError;
}

export interface TranscriptTurn {
  role: "user" | "assistant";
  text: string;
}

export interface TranscriptLogOptions {
  /** Newest turns kept for replay, total characters. Default 8000. */
  maxChars?: number;
  /** Newest turns kept for replay, count. Default 40. */
  maxItems?: number;
}

/**
 * The finalized turns of the current conversation, so a replacement
 * session can be given its context back. User turns only exist when the
 * session has input transcription on (`conversation.item.input_audio_
 * transcription.completed`); assistant turns come from
 * `response.output_audio_transcript.done`. Replayed as text items -- user
 * `input_text`, assistant `output_text`, the shapes LiveKit's
 * `livekit_item_to_openai_item` sends on its own reconnect.
 */
export class TranscriptLog {
  private readonly maxChars: number;
  private readonly maxItems: number;
  private turns: TranscriptTurn[] = [];

  constructor(opts: TranscriptLogOptions = {}) {
    this.maxChars = opts.maxChars ?? 8000;
    this.maxItems = opts.maxItems ?? 40;
  }

  add(role: TranscriptTurn["role"], text: unknown): void {
    if (typeof text !== "string") return;
    const t = text.trim();
    if (!t) return;
    this.turns.push({ role, text: t });
    // Trim as we go so a long session doesn't grow without bound.
    if (this.turns.length > this.maxItems * 2) this.turns = this.window();
  }

  clear(): void {
    this.turns = [];
  }

  get size(): number {
    return this.turns.length;
  }

  /** The newest turns within both budgets, oldest first. */
  window(): TranscriptTurn[] {
    const out: TranscriptTurn[] = [];
    let chars = 0;
    for (let i = this.turns.length - 1; i >= 0 && out.length < this.maxItems; i--) {
      const turn = this.turns[i];
      if (chars + turn.text.length > this.maxChars) break;
      chars += turn.text.length;
      out.push(turn);
    }
    return out.reverse();
  }

  /** `conversation.item.create` client events, in order, for a new session's data channel. */
  replayEvents(): Array<Record<string, unknown>> {
    return this.window().map((turn) => ({
      type: "conversation.item.create",
      item: {
        type: "message",
        role: turn.role,
        content: [{ type: turn.role === "user" ? "input_text" : "output_text", text: turn.text }],
      },
    }));
  }
}
