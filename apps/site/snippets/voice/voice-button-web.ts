import "@sinua/web/element"; // registers <sinua-view>
import type { SinuaViewElement } from "@sinua/web/element";
import { defineSinuaVoiceButtonElement, type SinuaVoiceButtonElement } from "@sinua/web/voice-button";
import { LocalMicVoiceSource } from "@sinua/voice/mic";

// <sinua-view pattern="glowing"></sinua-view>
// <sinua-voice-button mode="push-to-talk"></sinua-voice-button>   (or mode="toggle", the default)
defineSinuaVoiceButtonElement();

const source = new LocalMicVoiceSource(); // any VoiceSource: a vendor source, the mic, a simulation
document.querySelector<SinuaViewElement>("sinua-view")!.voice = source;
const button = document.querySelector<SinuaVoiceButtonElement>("sinua-voice-button")!;
button.source = source; // the button connects, mutes and ends the session; the view shows it

button.addEventListener("voicebuttonchange", (e) => {
  console.log("voice button:", e.detail.state); // ready, connecting, listening, muted, error
});
