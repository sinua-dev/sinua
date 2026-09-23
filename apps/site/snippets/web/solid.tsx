import "@sinua/web/element";
import type {} from "@sinua/web/types/solid";
import voiceOrb from "./voice-orb.fxspec.json";

// Solid: objects need `prop:`, events `on:`.
export function AssistantOrb(props: { state: string; muted: boolean }) {
  return (
    <sinua-view
      prop:spec={voiceOrb}
      state={props.state}
      prop:inputs={{ micMuted: props.muted ? 1 : 0 }}
      on:fxerror={(e: CustomEvent<unknown[]>) => console.warn(e.detail)}
      style={{ width: "160px" }}
    />
  );
}
