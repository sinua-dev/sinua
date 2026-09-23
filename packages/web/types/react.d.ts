// Opt-in React 19 typings for <sinua-view>: `import type {} from "@sinua/web/types/react"`.
// React 19 sets properties the element has and maps `onfxframe` to the `fxframe`
// event. (React users can also use `@sinua/web/react`'s <SinuaView/>.)
import type { DetailedHTMLProps, HTMLAttributes } from "react";
import type { FxFrameStats } from "@sinua/web";
import type { SinuaViewElementProps } from "@sinua/web/element";

declare module "react" {
  namespace JSX {
    interface IntrinsicElements {
      "sinua-view": DetailedHTMLProps<HTMLAttributes<HTMLElement>, HTMLElement> &
        SinuaViewElementProps & {
          onfxframe?: (event: CustomEvent<FxFrameStats>) => void;
          onfxerror?: (event: CustomEvent<unknown[]>) => void;
        };
    }
  }
}

export {};
