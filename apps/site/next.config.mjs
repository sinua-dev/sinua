import { createMDX } from "fumadocs-mdx/next";

const withMDX = createMDX();

/** @type {import('next').NextConfig} */
const config = {
  // @sinua/web and /core are `file:` deps: their symlinks leave apps/site, so
  // Turbopack needs the repo root to resolve them.
  turbopack: { root: new URL("../../", import.meta.url).pathname },
  // A static site (`npm run build` -> out/): no server at runtime.
  output: "export",
  // /docs/x/ -> docs/x/index.html: any static host (S3, GitHub Pages, python -m http.server)
  // serves it at the route the app hydrates for. Without it, a host that only serves
  // docs/x.html by its .html URL makes the router see another path: React #418 on every page.
  trailingSlash: true,
  // Markdown images (`![alt](/studio/x.png)`) become next/image, whose default
  // loader needs a server; a static export must opt out of optimisation.
  images: { unoptimized: true },
  reactStrictMode: true,
  // `next dev` would otherwise write AGENTS.md / CLAUDE.md into this folder.
  agentRules: false,
};

export default withMDX(config);
