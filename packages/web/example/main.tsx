// Three one-liners: a spec, a state + voice, and the React component.
import { createRoot } from "react-dom/client";
import type { AgentState, FxSpec, VoiceMetrics, VoiceSource } from "@sinua/core";
import { mount } from "../src/index";
import { SinuaView } from "../src/react";
import spec from "../../../spec/examples/voice-assistant.fxspec.json";

// A stand-in voice: speech-like bursts, no audio device (a real app passes
// its LocalMicVoiceSource / WebRTCVoiceSource / ... instead).
class FakeVoice implements VoiceSource {
  private m: ((m: VoiceMetrics) => void) | null = null;
  private s: ((s: AgentState) => void) | null = null;
  private id: ReturnType<typeof setInterval> | null = null;
  onMetrics(cb: (m: VoiceMetrics) => void) { this.m = cb; }
  onStateChange(cb: (s: AgentState) => void) { this.s = cb; }
  async connect() {
    const t0 = performance.now();
    this.id = setInterval(() => {
      const t = (performance.now() - t0) / 1000;
      const on = Math.sin(t * 2.1) > 0;
      const level = on ? 0.5 + 0.4 * Math.abs(Math.sin(t * 9)) : 0.03;
      this.m?.({ level, bands: Array.from({ length: 16 }, (_, i) => level * (0.4 + 0.6 * Math.abs(Math.sin(t * 5 + i)))) });
      this.s?.(on ? "speaking" : "listening");
    }, 1000 / 30);
  }
  disconnect() { if (this.id) clearInterval(this.id); this.s?.("idle"); }
}

mount(document.getElementById("spec") as HTMLCanvasElement, { spec: spec as FxSpec });

const voice = new FakeVoice();
mount(document.getElementById("voice") as HTMLCanvasElement, { state: "speaking", voice, label: "Assistant voice" });
void voice.connect();

createRoot(document.getElementById("react")).render(
  <>
    <SinuaView state="signaling" style={{ width: 160, height: 160 }} />
    <figcaption>{"<SinuaView state=\"signaling\" />"}</figcaption>
  </>
);
