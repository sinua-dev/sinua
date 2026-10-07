// The vendor-free half of the transcript API (docs/audio-pipeline.md, *Transcripts*):
// a source feeds it each vendor fragment and its 30 Hz level tick; it answers with
// cumulative `TranscriptUpdate`s, one turn at a time per speaker. DOM-free and
// clock-free (the caller passes `now`), so it runs under plain node and iOS and
// Android port it line for line, held to spec/transcript-cases.json.
//
// The rules:
// - `text` is everything visible so far in the turn; `final` comes exactly once per
//   turn; turn ids are role + a counter that never resets for this object
//   (a reconnect opens a new vendor session, not new ids).
// - Text is NFC-normalized fragment by fragment and joined as it came; only a final
//   text is trimmed, at its two ends. Lengths count Unicode code points.
// - User text shows as it arrives, in both modes.
// - Synced mode reveals assistant text with the played audio. `segments` (GPT-Live:
//   `start_ms`/`end_ms` on the session timeline) anchors each assistant turn at the
//   local audio onset (the first audible tick) mapped to the turn's first `start_ms`, and
//   reveals SEGMENT_DELAY_MS behind that timeline (it runs ahead of the played audio);
//   `none` reveals at REVEAL_CHARS_PER_SECOND while the audio is audible; `synced`
//   and `chars`-less fallbacks show text as it arrives. Text can't show before it
//   arrives: late text catches up at once. Never more than CUT_GRACE_MS past the
//   last audible moment, and not past it at all once the user talks over the audio
//   (`hold`, until the audio is heard again): a barge-in shows no unspoken word.
// - Raw mode (`syncToAudio: false`) shows vendor text as it arrives.
// - An assistant turn ends when the source leaves `speaking` (after its quiet tail),
//   or `truncated` on a barge-in: the text at the last audible moment (raw mode: the
//   fragments that ended by then, on the vendor's timeline through the same anchor).
//   Text after the cut that starts NEW_UTTERANCE_GAP_MS later is the next utterance
//   (the model talked on): it opens the next turn; the rest, the unspoken tail, goes.
// - A user turn ends after USER_SILENCE_MS without a fragment, or once an assistant
//   turn opened after it reaches USER_CLOSE_CHARS of raw text (a "mhm" doesn't).
import type { TranscriptTiming, TranscriptUpdate } from "@sinua/core";

export const TRANSCRIPT_USER_SILENCE_MS = 4000;
export const TRANSCRIPT_USER_CLOSE_CHARS = 12;
export const TRANSCRIPT_REVEAL_CHARS_PER_SECOND = 14;
export const TRANSCRIPT_CUT_GRACE_MS = 150;
/**
 * `segments`: GPT-Live's text timeline runs ahead of the played audio; the reveal waits this
 * long behind it (measured live, design note 39: centres the offset against the audio).
 */
export const TRANSCRIPT_SEGMENT_DELAY_MS = 300;
/**
 * A barge-in cut: text left over that starts at least this long after the kept text ends (on
 * the vendor's timeline) is the model's next utterance, not the unspoken tail: it opens the
 * next assistant turn instead of being dropped (GPT-Live talks on in full duplex).
 */
export const TRANSCRIPT_NEW_UTTERANCE_GAP_MS = 600;
/** The level above which the assistant's audio counts as audible (the sessions' speaking level). */
export const TRANSCRIPT_AUDIBLE_LEVEL = 0.05;
/** A tick gap longer than this doesn't count as speaking time (a stalled tab). */
const MAX_TICK_MS = 100;

interface Fragment {
  text: string;
  /** Code points in `text`. */
  length: number;
  startMs?: number;
  endMs?: number;
}

interface Turn {
  id: string;
  fragments: Fragment[];
  /** Code points of all fragments. */
  length: number;
  /** Code points shown so far. */
  shown: number;
  /** When the last fragment arrived (ms). */
  lastAt: number;
  /** Assistant: the local time of the audio onset and the vendor time it maps to. */
  onset: number | null;
  anchor: number | null;
  /** Assistant, `none`: audible seconds so far. */
  spoken: number;
  /** User: the assistant counter when it opened (a later assistant turn may close it). */
  assistantAtOpen: number;
}

const codePoints = (s: string): number => {
  let n = 0;
  for (const _ of s) n++;
  return n;
};

const prefix = (fragments: Fragment[], n: number): string => {
  let out = "";
  let left = n;
  for (const f of fragments) {
    if (left <= 0) break;
    if (f.length <= left) {
      out += f.text;
      left -= f.length;
    } else {
      out += Array.from(f.text).slice(0, left).join("");
      left = 0;
    }
  }
  return out;
};

export class TranscriptAssembler {
  onUpdate: ((u: TranscriptUpdate) => void) | null = null;
  private user: Turn | null = null;
  private assistant: Turn | null = null;
  private userCount = 0;
  private assistantCount = 0;
  /** Audio heard before the assistant turn's first fragment: its onset (ms). */
  private pendingOnset: number | null = null;
  private lastAudible: number | null = null;
  private lastTick: number | null = null;
  /** The user is talking over the audio: no reveal past the last audible moment. */
  private held = false;

  /** `sync`: reveal assistant text with the audio (`syncToAudio`); else raw. */
  constructor(
    readonly timing: TranscriptTiming,
    readonly sync: boolean,
  ) {}

  /** A user fragment (ASR delta), at `now` ms. */
  userDelta(text: string, now: number, startMs?: number, endMs?: number): void {
    if (!text) return;
    if (!this.user) this.user = this.open("user", now);
    this.append(this.user, text, now, startMs, endMs);
    this.user.shown = this.user.length;
    this.emit("user", this.user, false);
  }

  /** An assistant fragment, at `now` ms. */
  assistantDelta(text: string, now: number, startMs?: number, endMs?: number): void {
    if (!text) return;
    if (!this.assistant) {
      this.assistant = this.open("assistant", now);
      this.assistant.onset = this.pendingOnset;
    }
    const a = this.assistant;
    this.append(a, text, now, startMs, endMs);
    if (a.anchor == null && startMs != null) a.anchor = startMs;
    // A new assistant reply long enough ends the user's turn (raw text, not what's shown).
    const u = this.user;
    if (u && this.assistantCount > u.assistantAtOpen && a.length >= TRANSCRIPT_USER_CLOSE_CHARS) this.finish("user");
    this.reveal(a, now);
  }

  /**
   * The user started talking over the assistant's audio (a possible barge-in): the
   * reveal stops at the last audible moment until the audio is heard again.
   */
  hold(): void {
    if (this.assistant) this.held = true;
  }

  /** 30 Hz: the assistant audio's level and whether the source is in `speaking`, at `now` ms. */
  tick(now: number, level: number, speaking: boolean): void {
    const dt = this.lastTick == null ? 0 : Math.min(MAX_TICK_MS, Math.max(0, now - this.lastTick));
    this.lastTick = now;
    const audible = speaking && level > TRANSCRIPT_AUDIBLE_LEVEL;
    if (audible) {
      this.lastAudible = now;
      this.held = false;
      const a = this.assistant;
      if (a) {
        if (a.onset == null) a.onset = now;
        a.spoken += dt / 1000;
      } else if (this.pendingOnset == null) {
        this.pendingOnset = now;
      }
    } else if (!speaking) {
      this.pendingOnset = null;
    }
    if (this.user && now - this.user.lastAt >= TRANSCRIPT_USER_SILENCE_MS) this.finish("user");
    const a = this.assistant;
    if (a) {
      // Text with no audio at all (no onset) ends like a user turn: after the silence.
      if (a.onset == null && !speaking && now - a.lastAt >= TRANSCRIPT_USER_SILENCE_MS) this.finish("assistant");
      else this.reveal(a, now);
    }
  }

  /** The source left `speaking` on its own (its quiet tail ran out): the assistant turn ends. */
  speakingEnded(): void {
    const a = this.assistant;
    if (!a || a.onset == null) return;
    this.finish("assistant");
  }

  /** A barge-in cut the assistant: its turn ends with what was audible by the last audible moment. */
  cut(): void {
    const a = this.assistant;
    if (!a) return;
    const at = this.lastAudible;
    let n = 0;
    if (at != null && a.onset != null && at >= a.onset) {
      if (!this.sync && a.anchor != null) {
        // Raw: the fragments that ended by the cut, on the vendor's timeline.
        const vendorAt = a.anchor + (at - a.onset - TRANSCRIPT_SEGMENT_DELAY_MS);
        for (const f of a.fragments) {
          if (f.endMs == null || f.endMs > vendorAt) break;
          n += f.length;
        }
      } else {
        n = Math.min(a.shown, this.visible(a, at, at));
      }
    }
    a.shown = n;
    this.assistant = null;
    this.pendingOnset = null;
    this.held = false;
    this.emitText("assistant", a, prefix(a.fragments, n).trim(), true, true);
    // What follows the kept text: the unspoken tail (dropped), or, from the first fragment that
    // starts a gap after the one before it, the next utterance.
    let kept = 0;
    let i = 0;
    while (i < a.fragments.length && kept < n) kept += a.fragments[i++].length;
    const fs = a.fragments;
    const next = fs.findIndex((f, k) => k >= Math.max(i, 1) && f.startMs != null && fs[k - 1].endMs != null && f.startMs - (fs[k - 1].endMs as number) >= TRANSCRIPT_NEW_UTTERANCE_GAP_MS);
    if (next >= 0) {
      const t = this.open("assistant", a.lastAt);
      for (const f of a.fragments.slice(next)) this.append(t, f.text, a.lastAt, f.startMs, f.endMs);
      t.anchor = t.fragments[0].startMs ?? null;
      this.assistant = t;
      const u = this.user;
      if (u && this.assistantCount > u.assistantAtOpen && t.length >= TRANSCRIPT_USER_CLOSE_CHARS) this.finish("user");
    }
  }

  /** The session ended: open turns end as they are. */
  stop(): void {
    if (this.user) this.finish("user");
    if (this.assistant) {
      const a = this.assistant;
      this.assistant = null;
      this.emitText("assistant", a, prefix(a.fragments, a.shown).trim(), true, false);
    }
    this.pendingOnset = null;
    this.lastAudible = null;
    this.lastTick = null;
    this.held = false;
  }

  private open(role: "user" | "assistant", now: number): Turn {
    const id = role === "user" ? `u${++this.userCount}` : `a${++this.assistantCount}`;
    return { id, fragments: [], length: 0, shown: 0, lastAt: now, onset: null, anchor: null, spoken: 0, assistantAtOpen: this.assistantCount };
  }

  private append(t: Turn, text: string, now: number, startMs?: number, endMs?: number): void {
    const norm = text.normalize("NFC");
    const length = codePoints(norm);
    t.fragments.push({ text: norm, length, startMs, endMs });
    t.length += length;
    t.lastAt = now;
  }

  /** Code points of `a` visible at `now` (the playhead held within CUT_GRACE_MS of `audibleAt`). */
  private visible(a: Turn, now: number, audibleAt: number | null): number {
    if (!this.sync || this.timing === "synced" || this.timing === "chars") return a.length;
    if (a.onset == null) return 0;
    if (this.timing === "none") return Math.min(a.length, Math.floor(a.spoken * TRANSCRIPT_REVEAL_CHARS_PER_SECOND));
    // segments
    if (a.anchor == null) return a.length;
    const held = audibleAt == null ? now : Math.min(now, audibleAt + (this.held ? 0 : TRANSCRIPT_CUT_GRACE_MS));
    const head = a.anchor + (held - a.onset - TRANSCRIPT_SEGMENT_DELAY_MS);
    let n = 0;
    for (const f of a.fragments) {
      if (f.startMs == null || f.endMs == null) {
        n += f.length;
        continue;
      }
      if (head >= f.endMs) n += f.length;
      else {
        if (head > f.startMs) n += Math.floor((f.length * (head - f.startMs)) / (f.endMs - f.startMs));
        break;
      }
    }
    return n;
  }

  private reveal(a: Turn, now: number): void {
    const n = Math.max(a.shown, Math.min(a.length, this.visible(a, now, this.lastAudible)));
    if (n === a.shown) return;
    a.shown = n;
    this.emit("assistant", a, false);
  }

  private finish(role: "user" | "assistant"): void {
    const t = role === "user" ? this.user : this.assistant;
    if (!t) return;
    if (role === "user") this.user = null;
    else {
      this.assistant = null;
      this.pendingOnset = null;
      this.held = false;
    }
    t.shown = t.length;
    this.emitText(role, t, prefix(t.fragments, t.length).trim(), true, false);
  }

  private emit(role: "user" | "assistant", t: Turn, final: boolean): void {
    this.emitText(role, t, prefix(t.fragments, t.shown), final, false);
  }

  private emitText(role: "user" | "assistant", t: Turn, text: string, final: boolean, truncated: boolean): void {
    if (!final && !text.trim()) return;
    const u: TranscriptUpdate = { role, text, final, turnId: t.id };
    if (truncated) u.truncated = true;
    // The vendor's timing of the shown text: the first fragment's start, the last shown one's end.
    let seen = 0;
    let end: number | undefined;
    for (const f of t.fragments) {
      if (seen >= t.shown) break;
      seen += f.length;
      if (f.endMs != null) end = f.endMs;
    }
    const start = t.fragments[0]?.startMs;
    if (start != null && t.shown > 0) u.startMs = start;
    if (end != null) u.endMs = end;
    this.onUpdate?.(u);
  }
}
