import { Familjen_Grotesk, Newsreader } from "next/font/google";

/**
 * The landing and gallery type, loaded once and shared. The docs keep the
 * Fumadocs defaults, so these only apply inside `.lp`.
 */
export const display = Familjen_Grotesk({ subsets: ["latin"], weight: ["500", "600"], variable: "--lp-display" });
export const text = Newsreader({ subsets: ["latin"], weight: ["300", "400"], style: ["normal", "italic"], variable: "--lp-text" });

/** The class names every page outside the docs wraps its content in. */
export const siteShell = `lp ${display.variable} ${text.variable}`;
