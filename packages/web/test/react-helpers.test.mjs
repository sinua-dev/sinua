// SinuaVoiceMessage (a helper), rendered to static markup:
// the structure, sizing and accessibility. The canvas itself is mount()'s, tested in
// mount.test.mjs; effects don't run under renderToStaticMarkup.
import { test } from "node:test";
import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { playbackSeekProgress } from "@sinua/core";
import { SinuaVoiceMessage, SinuaView, voiceMessageSeek } from "../dist/react.js";

test("SinuaVoiceMessage: a slider with the position, over a box-layout canvas", () => {
  const html = renderToStaticMarkup(
    createElement(SinuaVoiceMessage, { envelope: [0.2, 0.8], progress: 0.42, onSeek: () => {}, style: { width: 220, height: 40 } }),
  );
  assert.match(html, /role="slider"/);
  assert.match(html, /aria-label="Voice message"/);
  assert.match(html, /aria-valuenow="42"/);
  assert.match(html, /tabindex="0"/);
  assert.doesNotMatch(html, /aspect-ratio:1/, "the dune fills its box, no square default");
});

test("voiceMessageSeek maps a touch onto the dune the engine draws", () => {
  assert.equal(voiceMessageSeek(200, 40, 0), 0, "left of the dune");
  assert.equal(voiceMessageSeek(200, 40, 200), 1, "right of the dune");
  assert.equal(voiceMessageSeek(200, 40, 100), playbackSeekProgress(5, 2.5));
  assert.ok(Math.abs(voiceMessageSeek(200, 40, 100) - 0.5) < 1e-9, "the middle of the box is the middle of the dune");
  assert.equal(voiceMessageSeek(0, 40, 10), 0, "no box yet");
});

test("SinuaView: a box-layout pattern gets no square default; others do", () => {
  assert.doesNotMatch(renderToStaticMarkup(createElement(SinuaView, { pattern: "dune" })), /aspect-ratio/);
  assert.match(renderToStaticMarkup(createElement(SinuaView, { pattern: "breathing" })), /aspect-ratio:1/);
});
