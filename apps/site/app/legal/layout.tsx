import type { ReactNode } from "react";
import { siteShell } from "@/lib/fonts";
import { SiteFooter, SiteHeader } from "@/components/chrome";
import "../landing.css";

/** Terms, privacy and refunds: site pages with the landing's chrome, not docs pages. */
export default function LegalLayout({ children }: { children: ReactNode }) {
  return (
    <div className={siteShell}>
      <SiteHeader />
      <main className="lp-main lp-legal">{children}</main>
      <SiteFooter />
    </div>
  );
}
