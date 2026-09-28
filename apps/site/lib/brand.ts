/**
 * Every brand string on the site, in one place: nothing outside this file
 * spells the product name out, so a rename stays one edit here (that is how
 * the Sinua rename was done).
 *
 * The copy claims only what is true of the published beta: no vendor adapter
 * has been verified against a live session, so none is said to "work with" one.
 */
export const brand = {
  /** The product, as people say it. */
  name: "Sinua",
  /** How the wordmark is set (the landing page and the docs nav). */
  wordmark: "Sinua",
  /** The hosted design tool, a separate product from the open-source runtime. */
  studioName: "Studio",
  /** The canonical origin: metadata, sitemap and robots are built from it. */
  siteUrl: "https://sinua.dev",
  /** One sentence for search results and link previews. */
  description:
    "Live visual primitives for voice AI. One Rust engine draws the same frame on the web, iOS, Android and React Native.",

  /** npm / SwiftPM / Maven names, shown in install lines and code samples. */
  packages: {
    web: "@sinua/web",
    core: "@sinua/core",
    reactNative: "@sinua/react-native",
    swift: "sinua-swift",
    android: "dev.sinua:sinua-view",
  },
  /** npm's `latest` still points at the first beta, so every npm line names the tag. */
  npmTag: "beta",
  /** The component prefix the catalog generates (scripts/codegen/config.mjs owns the real one). */
  componentPrefix: "Sinua",

  links: {
    docs: "/docs/getting-started/",
    gallery: "/gallery/",
    repo: "https://github.com/sinua-dev/sinua",
    /** The Studio isn't open yet, so the site links only to its tour, never to the tool. */
    studioTour: "/docs/studio/",
  },

  copy: {
    /** The one-line positioning. */
    tagline: "Live visuals for voice AI.",
    heroLead:
      "One engine draws the same frame on the web, iOS, Android and React Native. Give it your agent's state and it shows what is happening: listening, thinking, speaking.",
    useCasesLead: "The same engine, shaped for what your app is doing.",
    useCases: [
      { title: "A voice agent on screen", line: "The orb follows the mic while it listens, moves between turns and speaks with the agent's audio, from the agent state you already have." },
      { title: "Work in progress", line: "Rings fill from the numbers your app already tracks: an upload, a workout, a download." },
      { title: "Something being recorded", line: "A waveform that follows the level it is given, on a phone or in a browser tab." },
    ],
    frameworksLead: "One design, written once. The same props in React, SwiftUI, Compose and every web framework.",
    pricingLead: "The runtime is open source and free. The Studio, a design tool for it, is a separate product and not open yet.",
    pricingFree: "Packages",
    pricingPaid: "Studio",
    packagesPitch: "The engine and every renderer, Apache-2.0, shipped inside your app. Nothing to sign up for.",
    studioPitch: "Tune a visual live and export it as code or an FX Spec file. Not open yet.",
    footerNote: "Apache-2.0. Built for voice-AI interfaces.",
  },
} as const;

export type Brand = typeof brand;
