import type { MetadataRoute } from "next";
import { brand } from "@/lib/brand";
import { legalSource, source } from "@/lib/source";

// A static export writes this once, at build time, as out/sitemap.xml.
export const dynamic = "force-static";

export default function sitemap(): MetadataRoute.Sitemap {
  // trailingSlash is on (next.config.mjs), so every URL ends in "/", the form the host serves.
  const url = (path: string) => new URL(path.endsWith("/") ? path : `${path}/`, brand.siteUrl).toString();
  return [
    { url: url("/"), priority: 1 },
    { url: url(brand.links.gallery), priority: 0.8 },
    { url: url(brand.links.studioPage), priority: 0.7 },
    ...source.getPages().map((page) => ({ url: url(page.url), priority: 0.6 })),
    ...legalSource.getPages().map((page) => ({ url: url(page.url), priority: 0.2 })),
  ];
}
