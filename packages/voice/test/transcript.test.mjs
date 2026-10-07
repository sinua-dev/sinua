// The transcript rules (design note 39): spec/transcript-cases.json, which iOS and
// Android run too, through OpenAILiveSession with GPT-Live events and level ticks.
// No device, no network, no sound.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const table = JSON.parse(readFileSync(new URL("../../../spec/transcript-cases.json", import.meta.url), "utf8"));

test("TranscriptAssembler + OpenAILiveSession: spec/transcript-cases.json", async () => {
  const { OpenAILiveSession } = await import("../dist/openaiLive.js");
  const t = await import("../dist/transcript.js");
  assert.deepEqual(table.constants, {
    userSilenceMs: t.TRANSCRIPT_USER_SILENCE_MS,
    userCloseChars: t.TRANSCRIPT_USER_CLOSE_CHARS,
    revealCharsPerSecond: t.TRANSCRIPT_REVEAL_CHARS_PER_SECOND,
    cutGraceMs: t.TRANSCRIPT_CUT_GRACE_MS,
    segmentDelayMs: t.TRANSCRIPT_SEGMENT_DELAY_MS,
    newUtteranceGapMs: t.TRANSCRIPT_NEW_UTTERANCE_GAP_MS,
    audibleLevel: t.TRANSCRIPT_AUDIBLE_LEVEL,
  });
  assert.ok(table.cases.length >= 12);
  for (const c of table.cases) {
    const s = new OpenAILiveSession({ syncToAudio: c.syncToAudio });
    const last = { user: null, assistant: null };
    const finalIds = new Set();
    const shown = new Map();
    let finals = 0;
    s.onTranscript = (u) => {
      // Nothing after a final; text only grows, except in the truncated update.
      assert.ok(!finalIds.has(u.turnId), `${c.name}: update after ${u.turnId}'s final`);
      if (!u.truncated && !u.final) assert.ok(u.text.startsWith(shown.get(u.turnId) ?? ""), `${c.name}: ${u.turnId} shrank`);
      shown.set(u.turnId, u.text);
      if (u.final) {
        finalIds.add(u.turnId);
        finals++;
      }
      last[u.role] = u;
    };
    s.connecting();
    c.steps.forEach((step, i) => {
      const at = `${c.name}, step ${i + 1}`;
      if (step.event) s.handle(JSON.stringify(step.event), step.t);
      else if (step.reconnect) s.connecting();
      else for (let k = 0; k < (step.repeat ?? 1); k++) s.tick(step.level, step.t + k * 33);
      assert.deepEqual(last.user, step.user, `${at}: user`);
      assert.deepEqual(last.assistant, step.assistant, `${at}: assistant`);
      assert.equal(finals, step.finals, `${at}: finals`);
    });
  }
});

test("raw and synced share turn ids and finals; only the timing differs", async () => {
  const { OpenAILiveSession } = await import("../dist/openaiLive.js");
  for (const sync of [true, false]) {
    const s = new OpenAILiveSession({ syncToAudio: sync });
    const got = [];
    s.onTranscript = (u) => got.push(u);
    s.connecting();
    s.handle(JSON.stringify({ type: "session.started", session: { id: "x" } }), 0);
    s.handle(JSON.stringify({ type: "session.output_transcript.delta", delta: "Merhaba.", start_ms: 100, end_ms: 600 }), 10);
    for (let k = 0; k < 20; k++) s.tick(0.5, 20 + k * 33);
    for (let k = 0; k < 60; k++) s.tick(0, 700 + k * 33);
    const finals = got.filter((u) => u.final);
    assert.deepEqual(finals.map((u) => [u.turnId, u.text]), [["a1", "Merhaba."]], `sync ${sync}`);
  }
});

// T1b: the vendor-free half on its own (chars, none paced over received audio, explicit user
// end, keyed segments): spec/transcript-assembler-cases.json, which iOS and Android run too.
test("TranscriptAssembler: spec/transcript-assembler-cases.json", async () => {
  const t = await import("../dist/transcript.js");
  const ops = JSON.parse(readFileSync(new URL("../../../spec/transcript-assembler-cases.json", import.meta.url), "utf8"));
  assert.deepEqual(ops.constants, { rateMinAudioMs: t.TRANSCRIPT_RATE_MIN_AUDIO_MS });
  assert.ok(ops.cases.length >= 9);
  for (const c of ops.cases) {
    const a = new t.TranscriptAssembler(c.timing, c.syncToAudio, c.explicitUserEnd);
    const last = { user: null, assistant: null };
    let finals = 0;
    a.onUpdate = (u) => {
      last[u.role] = u;
      if (u.final) finals++;
    };
    c.steps.forEach((s, i) => {
      if (s.op === "tick") for (let k = 0; k < s.repeat; k++) a.tick(s.t + k * 33, s.level, s.speaking);
      else if (s.op === "user") a.userDelta(s.text, s.t);
      else if (s.op === "assistant") a.assistantDelta(s.text, s.t, s.startMs, s.endMs);
      else if (s.op === "userDone") a.userDone(s.text, s.t);
      else if (s.op === "audio") a.assistantAudio(s.ms);
      else if (s.op === "segment") a.segment(s.role, s.key, s.text, s.final, s.t);
      else if (s.op === "hold") a.hold();
      else if (s.op === "cut") a.cut();
      else if (s.op === "ended") a.speakingEnded();
      else if (s.op === "stop") a.stop();
      else assert.fail(`unknown op ${s.op}`);
      const at = `${c.name}, step ${i + 1}`;
      assert.deepEqual(last.user, s.user, `${at}: user`);
      assert.deepEqual(last.assistant, s.assistant, `${at}: assistant`);
      assert.equal(finals, s.finals, `${at}: finals`);
    });
  }
});
