/**
 * Every brand string on the site, in one place: nothing outside this file
 * spells the product name out, so a rename stays one edit here (that is how
 * the Sinua rename was done).
 *
 * Marketing copy is still being written. Anything unfinished is marked
 * `PLACEHOLDER` below: grep for it to find every line that needs real words.
 */
const PLACEHOLDER = (text: string) => text;

export const brand = {
  /** The product, as people say it. */
  name: "Sinua",
  /** How the wordmark is set (the landing page and the docs nav). */
  wordmark: "Sinua",
  /** The paid, hosted design tool. */
  studioName: "Studio",

  /** npm / SwiftPM / Maven names, shown in install lines and code samples. */
  packages: {
    web: "@sinua/web",
    core: "@sinua/core",
    reactNative: "@sinua/react-native",
    swift: "Sinua",
    android: "dev.sinua:view",
  },
  /** The component prefix the catalog generates (scripts/codegen/config.mjs owns the real one). */
  componentPrefix: "Sinua",

  links: {
    docs: "/docs/getting-started/",
    gallery: "/gallery/",
    /** Public repo URL -- empty while the repo is private. */
    repo: "",
  },

  copy: {
    /** The one-line positioning. */
    tagline: PLACEHOLDER("Design once. Render everywhere."),
    heroLead: PLACEHOLDER(
      "One engine draws the same frame on the web, iOS, Android and React Native. Give it your app's state and it shows what is happening: listening, thinking, speaking."
    ),
    useCasesLead: PLACEHOLDER("The same engine, shaped for what your app is doing."),
    useCases: [
      { title: PLACEHOLDER("A voice agent on screen"), line: PLACEHOLDER("The orb hears the mic, thinks between turns and speaks back, from the agent state you already have.") },
      { title: PLACEHOLDER("Work in progress"), line: PLACEHOLDER("Rings fill from the numbers your app already tracks: an upload, a workout, a download.") },
      { title: PLACEHOLDER("Something being recorded"), line: PLACEHOLDER("A waveform that follows the level it is given, on a phone or in a browser tab.") },
    ],
    frameworksLead: PLACEHOLDER("Pick a look in the Studio, paste it into whichever front end you use."),
    pricingLead: PLACEHOLDER("The runtime is open source. The design tool is the paid part."),
    pricingFree: PLACEHOLDER("Packages"),
    pricingPaid: PLACEHOLDER("Studio"),
    packagesPitch: PLACEHOLDER("Every renderer and the engine, Apache-2.0, self-hosted with your app."),
    studioPitch: PLACEHOLDER("A hosted tool to tune a visual live and export it as code or a spec file."),
    footerNote: PLACEHOLDER("Apache-2.0. Built for voice-AI interfaces."),
  },
} as const;

export type Brand = typeof brand;
