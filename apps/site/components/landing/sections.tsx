/**
 * What costs money: nothing in the runtime. The Studio is a separate, paid
 * product; this links to its page with prices, which links to the tool.
 */
import Link from "next/link";
import { brand } from "@/lib/brand";

export function Pricing() {
  return (
    <section className="lp-section" aria-labelledby="lp-pricing">
      <header className="lp-section-head">
        <h2 id="lp-pricing" className="lp-h2">
          Free runtime, separate Studio
        </h2>
        <p className="lp-lead">{brand.copy.pricingLead}</p>
      </header>
      <div className="lp-plans">
        <div className="lp-stage lp-plan">
          <h3 className="lp-h3">{brand.copy.pricingFree}</h3>
          <p>{brand.copy.packagesPitch}</p>
          <code className="lp-install">
            npm i {brand.packages.web}@{brand.npmTag}
          </code>
        </div>
        <div className="lp-stage lp-plan">
          <h3 className="lp-h3">
            {brand.copy.pricingPaid} {!brand.studioOpen && <em className="lp-pill">opening soon</em>}
          </h3>
          <p>
            {brand.copy.studioPitch} From {brand.studio.yearlyPerMonth} a month, with a {brand.studio.trialDays}-day free trial.
          </p>
          <div className="lp-actions lp-plan-actions">
            {brand.studioOpen ? (
              <a className="lp-button" href={`${brand.studio.url}/login`}>
                Start {brand.studio.trialDays}-day trial
              </a>
            ) : null}
            <Link className="lp-link" href={brand.links.studioPage}>
              See the Studio and prices
            </Link>
          </div>
        </div>
      </div>
    </section>
  );
}
