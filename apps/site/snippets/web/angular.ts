import { Component, CUSTOM_ELEMENTS_SCHEMA, input } from "@angular/core";
import "@sinua/web/element";
import voiceOrb from "./voice-orb.fxspec.json";

@Component({
  selector: "app-assistant-orb",
  standalone: true,
  // The one-line setup: allow the custom element. [prop] binds properties, (event) listens.
  schemas: [CUSTOM_ELEMENTS_SCHEMA],
  template: `
    <sinua-view [spec]="spec" [state]="state()" [inputs]="{ micMuted: muted() ? 1 : 0 }" (fxerror)="onError($event)" style="width: 160px"></sinua-view>
  `,
})
export class AssistantOrbComponent {
  spec = voiceOrb;
  state = input("listening");
  muted = input(false);
  onError(e: Event) {
    console.warn((e as CustomEvent).detail);
  }
}
