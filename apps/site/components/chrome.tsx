import Link from "next/link";
import { brand } from "@/lib/brand";

/**
 * The header and footer shared by every page outside the docs (the landing
 * page and the gallery); the docs get theirs from Fumadocs' DocsLayout.
 */
export function SiteHeader() {
  return (
    <header className="lp-top">
      <Link className="lp-wordmark" href="/">
        {brand.wordmark}
      </Link>
      <nav className="lp-nav">
        <Link href={brand.links.docs}>Docs</Link>
        <Link href={brand.links.gallery}>Gallery</Link>
      </nav>
    </header>
  );
}

export function SiteFooter() {
  return (
    <footer className="lp-footer">
      <Link className="lp-wordmark" href="/">
        {brand.wordmark}
      </Link>
      <nav className="lp-nav">
        <Link href={brand.links.docs}>Docs</Link>
        <Link href={brand.links.gallery}>Gallery</Link>
        {brand.links.repo ? <a href={brand.links.repo}>Source</a> : null}
      </nav>
      <p className="lp-foot-note">{brand.copy.footerNote}</p>
    </footer>
  );
}
