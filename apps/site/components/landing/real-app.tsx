"use client";
/**
 * Sinua in a real app: DevinFit, our own nutrition app, draws its Soul with it on
 * iPhone and Android. No voice there (the app has none): the same states run on app
 * data -- the colour follows the day's nutrition, and it thinks while the AI answers.
 *
 * The phones show screen recordings of the app itself, never a live Sinua visual
 * dressed up as one. A phone appears only once its recording is in public/real-app/;
 * with none, the section is the text and the code alone.
 * The file and the code beside them are the snippet files CI compiles.
 */
import { useEffect, useState } from "react";
import { Snippet } from "@sinua/design";
import { brand } from "@/lib/brand";
import { PlatformIcon, type Platform } from "./platform-icons";

export interface Recording {
  platform: Extract<Platform, "ios" | "android">;
  label: string;
  /** Public paths, when the recording exists. */
  video?: { webm?: string; mp4?: string; poster?: string };
}

const STATES = ["An empty day in the logo's colours", "The day's nutrition sets the colour", "Thinking while the AI answers"];

export function RealApp({ recordings, spec, swift, kotlin }: { recordings: Recording[]; spec: string; swift: string; kotlin: string }) {
  const recorded = recordings.filter((r) => r.video);
  return (
    <section className="lp-section" aria-labelledby="lp-real-app">
      <header className="lp-section-head">
        <h2 id="lp-real-app" className="lp-h2">
          In a real app
        </h2>
        <p className="lp-lead">
          DevinFit, our own nutrition app, draws its Soul with Sinua on iPhone and Android. Its colour follows the day&apos;s nutrition, and it thinks while
          the AI answers. There&apos;s no voice in DevinFit: the same states run on app data.
        </p>
      </header>

      <div className={recorded.length ? "lp-real" : "lp-real lp-real-solo"}>
        {recorded.length ? (
          <div className="lp-real-phones">
            {recorded.map((r) => (
              <figure key={r.platform} className="lp-device lp-real-phone" data-device={r.platform === "ios" ? "phone-ios" : "phone-android"}>
                <span className="lp-device-screen lp-real-screen">
                  <Recording video={r.video!} label={`DevinFit on ${r.label}, screen recording`} />
                </span>
                <figcaption className="lp-device-label">
                  <PlatformIcon platform={r.platform} />
                  {r.label}
                </figcaption>
              </figure>
            ))}
          </div>
        ) : null}

        <div className="lp-real-side">
          <ul className="lp-real-states" aria-label="What Soul shows">
            {STATES.map((s) => (
              <li key={s}>{s}</li>
            ))}
          </ul>
          <Snippet
            tabs={[
              { id: "spec", label: "soul.fxspec.json", code: spec },
              { id: "swiftui", label: "SwiftUI", code: swift },
              { id: "compose", label: "Compose", code: kotlin },
            ]}
          />
          <p className="lp-small">The file is the one DevinFit ships, as it is. The code is the shape of it: the app also writes the day&apos;s colour into the file before drawing it.</p>
          <div className="lp-actions lp-real-links">
            <a className="lp-link" href={brand.links.devinfitAppStore}>
              DevinFit on the App Store
            </a>
            <a className="lp-link" href={brand.links.devinfitWeb}>
              devinfit.app
            </a>
          </div>
        </div>
      </div>
    </section>
  );
}

/** A muted, looping recording. Under reduced motion it doesn't autoplay: the poster shows, with controls to play it. */
function Recording({ video, label }: { video: NonNullable<Recording["video"]>; label: string }) {
  // Read after hydration: the static HTML can't know the visitor's setting.
  const [reduced, setReduced] = useState(true);
  useEffect(() => {
    try {
      setReduced(window.matchMedia("(prefers-reduced-motion: reduce)").matches);
    } catch {
      setReduced(false);
    }
  }, []);
  return (
    <video className="lp-real-video" muted loop playsInline autoPlay={!reduced} controls={reduced} poster={video.poster} aria-label={label} preload="metadata">
      {video.webm ? <source src={video.webm} type="video/webm" /> : null}
      {video.mp4 ? <source src={video.mp4} type="video/mp4" /> : null}
    </video>
  );
}
