import type { ReactNode } from "react";
import { siteShell } from "@/lib/fonts";
import { SiteFooter, SiteHeader } from "@/components/chrome";
import "../landing.css";

/** The Studio's page (what it is, prices): a site page with the landing's chrome. */
export default function StudioLayout({ children }: { children: ReactNode }) {
  return (
    <div className={siteShell}>
      <SiteHeader />
      <main className="lp-main lp-studio">{children}</main>
      <SiteFooter />
    </div>
  );
}
