import Link from "next/link";
import { BrandMark } from "@sinua/design";
import { ThemeSwitch } from "fumadocs-ui/layouts/shared/slots/theme-switch";
import { brand } from "@/lib/brand";

/**
 * The header and footer shared by every page outside the docs (the landing
 * page and the gallery); the docs get theirs from Fumadocs' DocsLayout. The
 * theme switch is Fumadocs' own, so the choice carries between the two.
 */
export function SiteHeader() {
  return (
    <header className="lp-top">
      <Link className="lp-wordmark" href="/">
        <BrandMark className="lp-mark" />
        {brand.wordmark}
      </Link>
      <nav className="lp-nav" aria-label="Site">
        <Link href={brand.links.docs}>Docs</Link>
        <Link href={brand.links.gallery}>Gallery</Link>
        <Link href={brand.links.studioPage}>Studio</Link>
        <a href={brand.links.repo}>GitHub</a>
        <ThemeSwitch className="lp-theme" />
      </nav>
    </header>
  );
}

export function SiteFooter() {
  return (
    <footer className="lp-footer">
      <div className="lp-footer-row">
        <Link className="lp-wordmark" href="/">
          <BrandMark className="lp-mark" />
          {brand.wordmark}
        </Link>
        <nav className="lp-nav" aria-label="Footer">
          <Link href={brand.links.docs}>Docs</Link>
          <Link href={brand.links.gallery}>Gallery</Link>
          <Link href={brand.links.studioPage}>Studio</Link>
          <Link href="/docs/resources/changelog/">Changelog</Link>
          <Link href="/docs/resources/license/">License</Link>
          <Link href="/legal/terms/">Terms</Link>
          <Link href="/legal/privacy/">Privacy</Link>
          <Link href="/legal/refunds/">Refunds</Link>
          <a href={brand.links.repo}>GitHub</a>
        </nav>
      </div>
      <p className="lp-foot-note">
        {brand.copy.footerNote} Nine of the orb patterns are a port of{" "}
        <a href="https://github.com/Jakubantalik/Libraries.dev">thinking-orbs</a> by Jakub Antalik (MIT).
      </p>
      <p className="lp-foot-note lp-foot-legal">
        Platform marks belong to their owners and only name the platforms. The Android robot is reproduced or modified from work created and shared
        by Google and used according to terms described in the Creative Commons 3.0 Attribution License.
      </p>
    </footer>
  );
}
