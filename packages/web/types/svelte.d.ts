// Opt-in Svelte 5 typings for <sinua-view>: `import type {} from "@sinua/web/types/svelte"`
// in a .d.ts of your app. Svelte needs no other setup: it sets properties when the element has them.
import type { HTMLAttributes } from "svelte/elements";
import type { FxFrameStats } from "@sinua/web";
import type { SinuaViewElementProps } from "@sinua/web/element";

declare module "svelte/elements" {
  export interface SvelteHTMLElements {
    "sinua-view": HTMLAttributes<HTMLElement> &
      SinuaViewElementProps & {
        onfxframe?: (event: CustomEvent<FxFrameStats>) => void;
        onfxerror?: (event: CustomEvent<unknown[]>) => void;
      };
  }
}

export {};
