"use client";
/**
 * The voice side: where a voice comes from, what the engine takes from it, and
 * three different families following one simulated conversation in step. The
 * module names are the real products on each platform; the vendor sources are
 * labelled beta because none has been verified against its live service yet.
 */
import { useRef } from "react";
import { useConversation, SCRIPT } from "@/lib/conversation";
import { LiveVisual } from "./live-visual";
import { TINT } from "./swatches";
import { PlatformIcon, type Platform } from "./platform-icons";

const SOURCES: { name: string; note: string; beta?: boolean }[] = [
  { name: "Microphone", note: "the device mic, with no service at all" },
  { name: "LiveKit", note: "an agent in a LiveKit room", beta: true },
  { name: "OpenAI Realtime", note: "WebRTC", beta: true },
  { name: "Gemini Live", note: "WebSocket", beta: true },
  { name: "ElevenLabs", note: "Conversational AI", beta: true },
];

const MODULES: { id: Platform; platform: string; names: string[] }[] = [
  { id: "web", platform: "Web", names: ["@sinua/voice", "/livekit", "/openai", "/gemini", "/elevenlabs", "/mic"] },
  { id: "ios", platform: "iOS", names: ["SinuaVoice", "SinuaLiveKit", "SinuaOpenAI", "SinuaGeminiLive", "SinuaElevenLabs"] },
  { id: "android", platform: "Android", names: ["sinua-core", "sinua-livekit", "sinua-openai", "sinua-gemini", "sinua-elevenlabs"] },
];

const FOLLOWERS = [
  { pattern: "speaking", label: "Orb", hue: 265 },
  { pattern: "waveform", label: "Signal", hue: 172 },
  { pattern: "broadcasting", label: "Beacon", hue: 345 },
];

export function Voice() {
  const live = useRef<HTMLDivElement>(null);
  const convo = useConversation(live);
  const turn = SCRIPT[convo.turn];

  return (
    <section className="lp-section" aria-labelledby="lp-voice">
      <header className="lp-section-head">
        <h2 id="lp-voice" className="lp-h2">
          Wired to the conversation
        </h2>
        <p className="lp-lead">
          Hand a view a voice source and it follows the agent on its own: it listens while the user talks, thinks between turns and speaks with the
          agent&apos;s audio. All four states work on every pattern, and a barge-in shows as a brief flash.
        </p>
      </header>

      <div className="lp-voice">
        <div className="lp-stage lp-voice-flow">
          <h3 className="lp-h4">Where the voice comes from</h3>
          <ul className="lp-sources">
            {SOURCES.map((s) => (
              <li key={s.name}>
                <b>{s.name}</b>
                <span>{s.note}</span>
                {s.beta ? <em className="lp-pill">beta</em> : null}
              </li>
            ))}
          </ul>
          <p className="lp-flow-arrow" aria-hidden="true" />
          <p className="lp-flow-box">
            <code>VoiceSource</code>
            <span>the agent&apos;s state and the audio level, many times a second</span>
          </p>
          <p className="lp-small">
            Your server hands the app a short-lived token; a raw API key is always refused. The vendor sources are beta: tested on simulated
            sessions, not yet against each live service.
          </p>
        </div>

        <div ref={live} className="lp-voice-live">
          <ol className="lp-timeline" aria-label="One turn of the conversation">
            {SCRIPT.map((t, i) => (
              <li key={i} data-on={i === convo.turn} style={i === convo.turn ? { ["--p" as string]: convo.progress } : undefined}>
                <span>{t.state}</span>
              </li>
            ))}
          </ol>
          <div className="lp-followers">
            {FOLLOWERS.map((f) => (
              <figure key={f.pattern} className="lp-stage lp-follower">
                <LiveVisual
                  pattern={f.pattern}
                  state={convo.state}
                  level={convo.level}
                  maxFps={30}
                  overrides={{ ...TINT, colorHue: f.hue }}
                  className="lp-follower-visual"
                  label={`${f.label} ${f.pattern}, ${convo.state}`}
                />
                <figcaption>
                  {f.label} <span className="lp-muted">{f.pattern}</span>
                </figcaption>
              </figure>
            ))}
          </div>
          <p className="lp-caption lp-caption-quiet">
            {turn?.who ? <b>{turn.who}</b> : null}
            <span>{turn?.line?.slice(0, convo.shown) || " "}</span>
          </p>
          <dl className="lp-modules">
            {MODULES.map((m) => (
              <div key={m.platform}>
                <dt>
                  <PlatformIcon platform={m.id} size={14} />
                  {m.platform}
                </dt>
                <dd>
                  {m.names.map((n) => (
                    <code key={n}>{n}</code>
                  ))}
                </dd>
              </div>
            ))}
          </dl>
        </div>
      </div>
    </section>
  );
}
