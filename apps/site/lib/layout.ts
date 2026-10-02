/**
 * The frame a pattern is shown in. Most patterns draw in a square; the box-layout
 * one (`patternLayout()`: signal `playing`) fills whatever box it gets, so the site
 * gives it the box its use has: a chat bubble for a voice message.
 */
import { patternLayout } from "@sinua/core";

export type Shape = "square" | "wide";

export function shapeOf(pattern: string): Shape {
  return patternLayout(pattern) === "box" ? "wide" : "square";
}

/**
 * Inputs a pattern needs to look like itself in a preview: a voice message's
 * loudness (Sinua decodes nothing; an app passes up to 64 values) and how far it has
 * played. Empty for every other pattern.
 */
export function previewInputs(pattern: string): Record<string, number> {
  if (pattern !== "playing") return {};
  const out: Record<string, number> = { progress: 0.42 };
  for (let i = 0; i < 48; i++) {
    // A spoken sentence: phrases with pauses, syllables inside them.
    const phrase = Math.max(0, Math.sin((i / 48) * Math.PI * 3.2)) ** 0.6;
    out[`envelope${i}`] = Math.min(1, 0.12 + phrase * (0.55 + 0.35 * Math.abs(Math.sin(i * 1.7))));
  }
  return out;
}
