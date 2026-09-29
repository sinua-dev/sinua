import type { Metadata } from "next";
import Link from "next/link";
import { brand } from "@/lib/brand";

export const metadata: Metadata = {
  title: "Studio and pricing",
  description: `The ${brand.name} Studio: design a voice-AI visual in the browser and export it to Web, iOS, Android and React Native. ${brand.studio.monthly} a month or ${brand.studio.yearly} a year, ${brand.studio.trialDays} days free.`,
  alternates: { canonical: `${brand.links.studioPage}` },
};

const s = brand.studio;

/** Until the Studio opens, the plan buttons say so instead of linking to it (brand.studioOpen). */
function TrialButton() {
  return brand.studioOpen ? (
    <a className="lp-button" href={`${s.url}/login`}>
      Start {s.trialDays}-day trial
    </a>
  ) : (
    <span className="lp-button lp-button-off" aria-disabled="true">
      Opening soon
    </span>
  );
}

const FEATURES: [string, string][] = [
  ["Every prop, every material", "Appearance, motion and energy, then glow, noise, particles, liquid, holographic and pulse, with the advanced controls the basic export leaves out."],
  ["Colour and gradients", "Pick a colour or build a gradient and see it on light and dark, at every size."],
  ["States, bindings and transitions", "Give each state of your app its own look, bind your values to the visual, and set how one state flows into the next."],
  ["A live voice", "Preview with your microphone, a test tone or a simulated conversation, or connect an OpenAI, Gemini, ElevenLabs or LiveKit session (beta)."],
  ["My designs", "Save your designs to your account and open them on any computer."],
  ["Export anywhere", "Code for Web, SwiftUI, Jetpack Compose and React Native, an FX Spec file, or a PNG or SVG image."],
];

const QUESTIONS: [string, React.ReactNode][] = [
  [
    "Do I need a card for the trial?",
    <>Yes. The trial is free for {s.trialDays} days; cancel before it ends and you're never charged.</>,
  ],
  [
    "Can I cancel or get a refund?",
    <>
      Cancel any time from the Studio; you keep access until the period ends. If the Studio isn&apos;t for you after your first charge, ask within 14 days and
      we&apos;ll refund it in full. See the{" "}
      <Link href="/legal/refunds/">Refund Policy</Link>.
    </>,
  ],
  [
    "What happens to what I exported if I stop paying?",
    <>
      It keeps working. The code and FX Spec files you export are yours to use in any project, commercial or not, and the runtime they run on is open source. See the{" "}
      <Link href="/legal/terms/">Terms</Link>.
    </>,
  ],
  [
    "Who handles payment and tax?",
    <>Polar, our merchant of record. It charges you, sends the invoice and handles sales tax and VAT.</>,
  ],
  [
    `Do I need the Studio to use ${brand.name}?`,
    <>
      No. The runtime is free and open source, and every pattern works from code. The <Link href={brand.links.gallery}>gallery</Link> gives you the code, the FX
      Spec file and a prompt for your coding agent for each pattern, for free.
    </>,
  ],
];

export default function StudioPage() {
  return (
    <>
      <section className="lp-studio-hero" aria-labelledby="st-title">
        <h1 id="st-title" className="lp-h1">
          {brand.name} {brand.studioName}
        </h1>
        <p className="lp-lead">Design the visual in the browser, then take it to Web, iOS, Android and React Native.</p>
        <img className="lp-studio-shot" src="/studio/studio-tour.png" alt="The Studio: the live stage, its toolbar, the inspector and the export drawer" />
      </section>

      <section className="lp-section" aria-labelledby="st-plans">
        <header className="lp-section-head">
          <h2 id="st-plans" className="lp-h2">
            Pricing
          </h2>
          <p className="lp-lead">
            One Studio, two ways to pay. Both start with {s.trialDays} days free and cancel any time.
          </p>
        </header>
        <div className="lp-plans">
          <div className="lp-stage lp-plan">
            <h3 className="lp-h3">Monthly</h3>
            <p className="lp-price">
              <strong>{s.monthly}</strong> a month
            </p>
            <p>{s.trialDays}-day free trial, then billed monthly.</p>
            <TrialButton />
          </div>
          <div className="lp-stage lp-plan">
            <h3 className="lp-h3">Yearly</h3>
            <p className="lp-price">
              <strong>{s.yearly}</strong> a year
            </p>
            <p>
              {s.yearlyPerMonth} a month, billed yearly. {s.trialDays}-day free trial.
            </p>
            <TrialButton />
          </div>
        </div>
        {!brand.studioOpen && <p className="lp-studio-note">The Studio isn&apos;t open yet. Prices are final; sign-up opens soon.</p>}
      </section>

      <section className="lp-section" aria-labelledby="st-get">
        <header className="lp-section-head">
          <h2 id="st-get" className="lp-h2">
            What the Studio adds
          </h2>
          <p className="lp-lead">
            The runtime and the <Link href={brand.links.gallery}>gallery</Link> are free: every pattern, its code and its FX Spec file. The Studio is for tuning
            one to your product.
          </p>
        </header>
        <ul className="lp-studio-features">
          {FEATURES.map(([title, text]) => (
            <li key={title}>
              <h3 className="lp-h3">{title}</h3>
              <p>{text}</p>
            </li>
          ))}
        </ul>
        <Link className="lp-link" href={brand.links.studioTour}>
          Take the tour
        </Link>
      </section>

      <section className="lp-section" aria-labelledby="st-faq">
        <header className="lp-section-head">
          <h2 id="st-faq" className="lp-h2">
            Questions
          </h2>
        </header>
        <dl className="lp-studio-faq">
          {QUESTIONS.map(([q, a]) => (
            <div key={q}>
              <dt>{q}</dt>
              <dd>{a}</dd>
            </div>
          ))}
        </dl>
      </section>
    </>
  );
}
