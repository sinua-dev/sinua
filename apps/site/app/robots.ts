import type { MetadataRoute } from "next";
import { brand } from "@/lib/brand";

export const dynamic = "force-static";

export default function robots(): MetadataRoute.Robots {
  return {
    rules: { userAgent: "*", allow: "/" },
    sitemap: new URL("/sitemap.xml", brand.siteUrl).toString(),
  };
}
