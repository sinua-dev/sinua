// Opt-in Solid typings for <sinua-view>: `import type {} from "@sinua/web/types/solid"`.
// Objects go as properties -- `prop:spec={spec}`, `prop:inputs={{ … }}` -- and
// events with `on:fxframe={…}`; scalars can be plain attributes (`pattern="breathing"`).
import type { JSX } from "solid-js";
import type { FxFrameStats } from "@sinua/web";
import type { SinuaViewElementProps } from "@sinua/web/element";

declare module "solid-js" {
  namespace JSX {
    interface IntrinsicElements {
      "sinua-view": JSX.HTMLAttributes<HTMLElement> & {
        pattern?: string;
        state?: string;
        size?: 20 | 32 | 64 | `${20 | 32 | 64}`;
        speed?: number | string;
        theme?: "auto" | "light" | "dark";
        paused?: boolean;
        "reduced-motion"?: "auto" | "always" | "never";
        "max-fps"?: number | string;
        "low-power"?: boolean;
        label?: string;
        "voice-level-input"?: string;
        "cross-fade"?: number | string;
      };
    }
    interface ExplicitProperties {
      spec: SinuaViewElementProps["spec"];
      inputs: SinuaViewElementProps["inputs"];
      overrides: SinuaViewElementProps["overrides"];
      voice: SinuaViewElementProps["voice"];
    }
    interface CustomEvents {
      fxframe: CustomEvent<FxFrameStats>;
      fxerror: CustomEvent<unknown[]>;
    }
  }
}

export {};
