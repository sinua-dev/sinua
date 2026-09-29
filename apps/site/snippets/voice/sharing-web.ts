import { SharedVoiceSource, type VoiceSource } from "@sinua/core";

// A source holds one callback of each kind. To listen yourself next to a view or a
// button, subscribe through the shared fan-out, never on the raw source.
export function watch(source: VoiceSource, pill: HTMLElement) {
  const shared = SharedVoiceSource.of(source); // the same instance every time for this source
  const offState = shared.onStateChange((state) => (pill.textContent = state));
  const offLink = shared.onConnectionChange((connected) => pill.toggleAttribute("data-live", connected));
  return () => {
    offState();
    offLink();
  };
}

// Mute without ending the session: silence goes out, the agent keeps its context,
// and every view bound to the source shows the muted cue.
export const mute = (source: VoiceSource, muted: boolean) => SharedVoiceSource.of(source).setMuted(muted);
