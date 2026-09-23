import type { ReactNode } from "react";
import { siteShell } from "@/lib/fonts";
import { SiteFooter, SiteHeader } from "@/components/chrome";
import "../landing.css";

/** The gallery is a site page, not a docs page: it gets the same chrome as the landing. */
export default function GalleryLayout({ children }: { children: ReactNode }) {
  return (
    <div className={siteShell}>
      <SiteHeader />
      <main className="lp-main lp-gallery">{children}</main>
      <SiteFooter />
    </div>
  );
}
