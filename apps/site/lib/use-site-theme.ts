"use client";
import { useEffect, useState } from "react";

/**
 * The site's own theme, not the OS's: Fumadocs' toggle sets `class="dark"` on
 * <html>, while the engine's `theme: "auto"` would follow `prefers-color-scheme`.
 * Every live visual on the site passes this instead.
 */
export function useSiteTheme(): "light" | "dark" {
  const [dark, setDark] = useState(false);
  useEffect(() => {
    const read = () => setDark(document.documentElement.classList.contains("dark"));
    read();
    const mo = new MutationObserver(read);
    mo.observe(document.documentElement, { attributes: true, attributeFilter: ["class"] });
    return () => mo.disconnect();
  }, []);
  return dark ? "dark" : "light";
}
