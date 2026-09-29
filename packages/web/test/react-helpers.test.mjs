// SinuaAvatar and SinuaVoiceMessage (roadmap 7/8 helpers), rendered to static markup:
// the structure, sizing and accessibility. The canvas itself is mount()'s, tested in
// mount.test.mjs; effects don't run under renderToStaticMarkup.
import { test } from "node:test";
import assert from "node:assert/strict";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { playbackSeekProgress } from "@sinua/core";
import { SinuaAvatar, SinuaVoiceMessage, SinuaView, voiceMessageSeek, AVATAR_INNER_RADIUS } from "../dist/react.js";

test("SinuaAvatar: a circle image 2 x innerRadius of the box, centred, with a decorative ring", () => {
  const html = renderToStaticMarkup(createElement(SinuaAvatar, { src: "/a.png", alt: "Ada" }));
  assert.equal(AVATAR_INNER_RADIUS, 0.34);
  assert.match(html, /<img src="\/a.png" alt="Ada" style="[^"]*left:16.000%;top:16.000%;width:68.000%;height:68.000%;border-radius:50%/);
  assert.match(html, /aspect-ratio:1/, "a square box");
  assert.match(html, /<canvas[^>]*position:absolute/, "the ring over the whole box");
  const wide = renderToStaticMarkup(createElement(SinuaAvatar, { src: "/a.png", alt: "Ada", innerRadius: 0.4 }));
  assert.match(wide, /width:80.000%/);
});

test("SinuaVoiceMessage: a slider with the position, over a box-layout canvas", () => {
  const html = renderToStaticMarkup(
    createElement(SinuaVoiceMessage, { envelope: [0.2, 0.8], progress: 0.42, onSeek: () => {}, style: { width: 220, height: 40 } }),
  );
  assert.match(html, /role="slider"/);
  assert.match(html, /aria-label="Voice message"/);
  assert.match(html, /aria-valuenow="42"/);
  assert.match(html, /tabindex="0"/);
  assert.doesNotMatch(html, /aspect-ratio:1/, "the waveform fills its box, no square default");
});

test("voiceMessageSeek maps a touch onto the engine's row of bars", () => {
  assert.equal(voiceMessageSeek(200, 40, 0), 0, "left of the row");
  assert.equal(voiceMessageSeek(200, 40, 200), 1, "right of the row");
  assert.equal(voiceMessageSeek(200, 40, 100), playbackSeekProgress(5, 2.5));
  assert.ok(Math.abs(voiceMessageSeek(200, 40, 100) - 0.5) < 1e-9, "the middle of the box is the middle of the row");
  assert.equal(voiceMessageSeek(0, 40, 10), 0, "no box yet");
});

test("SinuaView: a box-layout pattern gets no square default; others do", () => {
  assert.doesNotMatch(renderToStaticMarkup(createElement(SinuaView, { pattern: "framing" })), /aspect-ratio/);
  assert.match(renderToStaticMarkup(createElement(SinuaView, { pattern: "breathing" })), /aspect-ratio:1/);
});
