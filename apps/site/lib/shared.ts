import { brand } from "./brand";

/** The docs nav title, from the one place the brand lives (lib/brand.ts). */
export const appName = brand.wordmark;
export const docsRoute = "/docs";
/** Where /docs and / land: content/docs has no root index page. */
export const firstPage = "/docs/getting-started";
