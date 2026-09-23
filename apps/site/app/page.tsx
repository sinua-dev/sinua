import { siteShell } from "@/lib/fonts";
import { SiteFooter, SiteHeader } from "@/components/chrome";
import { Hero } from "@/components/landing/hero";
import { Frameworks, Pricing, UseCases } from "@/components/landing/sections";
import "./landing.css";

export default function LandingPage() {
  return (
    <div className={siteShell}>
      <SiteHeader />
      <main className="lp-main">
        <Hero />
        <UseCases />
        <Frameworks />
        <Pricing />
      </main>
      <SiteFooter />
    </div>
  );
}
