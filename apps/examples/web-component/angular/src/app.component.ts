import { Component, CUSTOM_ELEMENTS_SCHEMA, signal } from "@angular/core";
import spec from "../../spec.json";

@Component({
  selector: "app-root",
  standalone: true,
  // Angular: the one-line setup -- allow the custom element; [prop] binds properties.
  schemas: [CUSTOM_ELEMENTS_SCHEMA],
  template: `
    @if (shown()) {
      <sinua-view id="fx" style="width: 192px" theme="light" [spec]="spec" [inputs]="inputs()" (fxframe)="frames.set(frames() + 1)"></sinua-view>
    }
    <p>frames: <span id="frames">{{ frames() }}</span></p>
    <button id="toggle" (click)="inputs.set({ micMuted: inputs().micMuted ? 0 : 1 })">toggle micMuted</button>
    <button id="remove" (click)="shown.set(false)">remove</button>
  `,
})
export class AppComponent {
  spec = spec;
  inputs = signal({ micMuted: 0 });
  frames = signal(0);
  shown = signal(true);
}
