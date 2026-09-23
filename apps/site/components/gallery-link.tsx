import Link from "next/link";

/**
 * A docs page's way into the gallery, filtered to what the page is about:
 * `<GalleryLink family="ring" />` or `<GalleryLink family="orb" pattern="glowing" />`.
 */
export function GalleryLink({ family, pattern, children }: { family: string; pattern?: string; children?: React.ReactNode }) {
  const query = new URLSearchParams({ family, ...(pattern ? { pattern } : {}) }).toString();
  return (
    <Link className="fx-gallery-link" href={`/gallery/?${query}`}>
      {children ?? `See every ${family} pattern live`}
    </Link>
  );
}
