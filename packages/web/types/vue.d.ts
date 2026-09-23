// Opt-in Vue typings for <sinua-view>: `import type {} from "@sinua/web/types/vue"`
// (or list it in tsconfig `types`). Vue also needs to know the tag is a custom
// element: `vue({ template: { compilerOptions: { isCustomElement: (t) => t === "sinua-view" } } })`.
import type { DefineComponent } from "vue";
import type { FxFrameStats } from "@sinua/web";
import type { SinuaViewElementProps } from "@sinua/web/element";

declare module "vue" {
  interface GlobalComponents {
    "sinua-view": DefineComponent<
      SinuaViewElementProps & {
        onFxframe?: (event: CustomEvent<FxFrameStats>) => void;
        onFxerror?: (event: CustomEvent<unknown[]>) => void;
      }
    >;
  }
}

export {};
