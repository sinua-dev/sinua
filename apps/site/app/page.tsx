import { existsSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { siteShell } from "@/lib/fonts";
import { SiteFooter, SiteHeader } from "@/components/chrome";
import { Hero } from "@/components/landing/hero";
import { Families } from "@/components/landing/families";
import { Platforms } from "@/components/landing/platforms";
import { Voice } from "@/components/landing/voice";
import { Look } from "@/components/landing/look";
import { Pricing } from "@/components/landing/sections";
import { RealApp, type Recording } from "@/components/landing/real-app";
import "./landing.css";

/**
 * The numbers the page states are counted from the repository at build time
 * (a static export: this runs once, in `next build`), so they can't go stale:
 * the patterns from the catalog, the frozen frames from the golden set the web,
 * iOS and Android builds each check.
 */
function counts() {
  const spec = (name: string): unknown => JSON.parse(readFileSync(join(process.cwd(), "../../spec", name), "utf8"));
  const catalog = spec("parameters.json") as { objects: { patterns: unknown[] }[] };
  const golden = spec("sinua-golden.json") as { cases: unknown[] };
  return { patterns: catalog.objects.reduce((n, o) => n + o.patterns.length, 0), frames: golden.cases.length };
}

/**
 * DevinFit's material, read at build time: the recordings that exist in
 * public/real-app/, and the snippet files CI compiles (snippets/real-app/).
 */
function realApp() {
  const recording = (platform: Recording["platform"], label: string): Recording => {
    const base = `real-app/devinfit-${platform}`;
    const has = (ext: string) => existsSync(join(process.cwd(), "public", `${base}.${ext}`));
    const video = has("webm") || has("mp4") ? { webm: has("webm") ? `/${base}.webm` : undefined, mp4: has("mp4") ? `/${base}.mp4` : undefined, poster: has("jpg") ? `/${base}.jpg` : undefined } : undefined;
    return { platform, label, video };
  };
  // Full snippets-relative paths, so the reachability check (scripts/docs/check-snippets.mjs) sees them.
  const snippet = (path: string) => readFileSync(join(process.cwd(), "snippets", path), "utf8").trimEnd();
  return {
    recordings: [recording("ios", "iPhone"), recording("android", "Android")],
    spec: snippet("real-app/soul.fxspec.json"),
    swift: snippet("real-app/soul.swift"),
    kotlin: snippet("real-app/soul.kt").replace(/^package .*\n\n/, ""),
  };
}

export default function LandingPage() {
  const { patterns, frames } = counts();
  const devinfit = realApp();
  return (
    <div className={siteShell}>
      <SiteHeader />
      <main className="lp-main">
        <Hero patternCount={patterns} />
        <Families />
        <Voice />
        <Platforms frames={frames} />
        <RealApp {...devinfit} />
        <Look />
        <Pricing />
      </main>
      <SiteFooter />
    </div>
  );
}
