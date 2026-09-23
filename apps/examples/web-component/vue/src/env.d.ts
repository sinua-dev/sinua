/// <reference types="vite/client" />
import type {} from "@sinua/web/types/vue";
declare module "*.vue" { import type { DefineComponent } from "vue"; const c: DefineComponent; export default c; }
