import { mount } from "@sinua/web";
import { LocalMicVoiceSource } from "@sinua/voice/mic";
import voiceOrb from "../spec/voice-orb.fxspec.json";

const canvas = document.querySelector<HTMLCanvasElement>("#orb")!;
const voice = new LocalMicVoiceSource();

// The view binds the source: level and bands drive the visual, and the source's
// AgentState picks the spec's lifecycle state. It never connects the source itself.
const fx = mount(canvas, { spec: voiceOrb, voice });

// Connect from a user gesture: the mic prompt and audio playback both need one.
document.querySelector("#talk")!.addEventListener("click", async () => {
  try {
    await voice.connect();
  } catch (err) {
    console.error("voice failed to start", err); // e.g. the mic permission was denied
  }
});

// On teardown: voice.disconnect(); fx.destroy();
export { fx, voice };
