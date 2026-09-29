import { mount } from "@sinua/web";
import { LocalMicVoiceSource } from "@sinua/voice/mic";

const canvas = document.querySelector<HTMLCanvasElement>("#assistant")!;

// The accessible name follows the state: "Coach", then "Coach, listening"…
// State changes are spoken politely; `announce: false` turns that off.
export const fx = mount(canvas, {
  pattern: "glowing",
  voice: new LocalMicVoiceSource(),
  label: "Coach",
  labels: { listening: "Koç dinliyor", speaking: "Koç konuşuyor" }, // your words, per state
  announce: true,
});
