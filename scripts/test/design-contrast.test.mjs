// The design tokens' contrast (packages/design/css/tokens.css): every text pair is at least 4.5:1
// and every non-text decoration pair at least 3:1, in both themes. The two themes use Aurora
// differently on purpose (dark: the bright pair; light: the deep inks), so a later token edit
// that breaks a pair, or puts the bright teal back under light text, fails here.
import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";

const css = readFileSync(new URL("../../packages/design/css/tokens.css", import.meta.url), "utf8");

/** The `--name: value;` declarations of the block that starts at `open`. */
function block(open) {
  const i = css.indexOf(open);
  assert.ok(i >= 0, `no block ${open}`);
  const body = css.slice(css.indexOf("{", i) + 1, css.indexOf("\n}", i));
  const out = {};
  for (const m of body.matchAll(/--([\w-]+):\s*([^;]+);/g)) out[m[1]] = m[2].trim();
  return out;
}
const light = block(":root {");
const dark = { ...light, ...block(':root[data-theme="dark"]') };

function resolve(theme, name) {
  let v = theme[name];
  assert.ok(v, `--${name} is not defined`);
  const ref = v.match(/^var\(--([\w-]+)\)$/);
  return ref ? resolve(theme, ref[1]) : v;
}
function rgb(hex) {
  const m = hex.match(/^#([0-9a-f]{6})$/i);
  assert.ok(m, `${hex} is not a #rrggbb colour`);
  return [0, 2, 4].map((i) => parseInt(m[1].slice(i, i + 2), 16) / 255);
}
function luminance(hex) {
  const [r, g, b] = rgb(hex).map((c) => (c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * r + 0.7152 * g + 0.0722 * b;
}
function ratio(a, b) {
  const [hi, lo] = [luminance(a), luminance(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}
function check(theme, label, fg, bg, min) {
  const r = ratio(resolve(theme, fg), resolve(theme, bg));
  assert.ok(r >= min, `${label}: --${fg} on --${bg} is ${r.toFixed(2)}:1, needs ${min}`);
}

for (const [name, theme] of [["light", light], ["dark", dark]]) {
  test(`${name}: text and Aurora text pairs are at least 4.5:1`, () => {
    for (const bg of ["bg-app", "bg-panel", "bg-stage", "bg-elevated"]) {
      check(theme, `${name} ink`, "text", bg, 4.5);
      check(theme, `${name} secondary ink`, "text-2", bg, 4.5);
      check(theme, `${name} Aurora ink`, "accent-ink", bg, 4.5);
    }
    // Buttons: the on-accent colour on both ends of Aurora.
    check(theme, `${name} button`, "on-accent", "accent", 4.5);
    check(theme, `${name} button, blue end`, "on-accent", "accent-2", 4.5);
    check(theme, `${name} gradient button, from`, "on-accent", "aurora-from", 4.5);
    check(theme, `${name} gradient button, to`, "on-accent", "aurora-to", 4.5);
    // The inverted selection pill.
    check(theme, `${name} pill`, "pill-text", "pill-bg", 4.5);
  });

  test(`${name}: decoration pairs are at least 3:1`, () => {
    for (const bg of ["bg-app", "bg-panel"]) {
      check(theme, `${name} --text-3`, "text-3", bg, 3);
      check(theme, `${name} strong border`, "border-strong", bg, 3);
      check(theme, `${name} Aurora decoration, from`, "aurora-deco-from", bg, 3);
      check(theme, `${name} Aurora decoration, to`, "aurora-deco-to", bg, 3);
    }
  });
}

test("the light theme never uses the bright teal as text, a link or a button", () => {
  const bright = "#2de2b5";
  for (const name of ["text", "text-2", "text-3", "accent", "accent-ink", "aurora-from"]) {
    assert.notEqual(resolve(light, name).toLowerCase(), bright, `light --${name} is the bright teal (1.66:1 on white)`);
  }
  // The bright teal on white really is unreadable: the rule above exists for a reason.
  assert.ok(ratio(bright, "#ffffff") < 2);
});

test("dark is pure black behind the app and the stage", () => {
  assert.equal(resolve(dark, "bg-app"), "#000000");
  assert.equal(resolve(dark, "bg-stage"), "#000000");
});
