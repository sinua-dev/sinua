// `<SinuaView/>` -- a thin React wrapper over `mount` (React is an optional
// peer of @sinua/web; the root entry never imports it).
//
//   import { SinuaView } from "@sinua/web/react";
//   <SinuaView spec={mySpec} voice={source} style={{ width: 160, height: 160 }} />
import { createElement, useEffect, useMemo, useRef, type CSSProperties } from "react";
import type { VoiceButtonLabels, VoiceButtonMode, VoiceSource } from "@sinua/core";
import { mount, viewLayout, type FxHandle, type SinuaViewOptions } from "./mount.js";
import { defineSinuaVoiceButtonElement, type SinuaVoiceButtonElement, type VoiceButtonChangeDetail } from "./voice-button.js";

export interface SinuaViewProps extends SinuaViewOptions {
  className?: string;
  style?: CSSProperties;
  /** The live handle (pause/resume, `voice` for a meter), once mounted. */
  onReady?: (handle: FxHandle) => void;
}

export function SinuaView({ className, style, onReady, ...options }: SinuaViewProps) {
  const ref = useRef<HTMLCanvasElement>(null);
  const handle = useRef<FxHandle | null>(null);
  const first = useRef(true);

  useEffect(() => {
    if (!ref.current) return;
    const h = mount(ref.current, options);
    handle.current = h;
    onReady?.(h);
    return () => {
      h.destroy();
      handle.current = null;
      first.current = true;
    };
    // Mount once per canvas; later prop changes go through update() below.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  // Push changed props. Object props (spec, overrides, inputs) are compared
  // by reference: memoize them if you build them inline.
  const { spec, pattern, state, size, overrides, speed, voice, voiceOptions, specState, inputs, voiceLevelInput, crossFade, theme, paused, reducedMotion, label, onError, maxFps, lowPower, onFrame, pointer, labels, announce, rules } =
    options;
  const deps = [spec, pattern, size, overrides, speed, voice, voiceOptions, crossFade, maxFps, lowPower, pointer];
  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    handle.current?.update({ spec, pattern, size, overrides, speed, voice, voiceOptions, crossFade, maxFps, lowPower, pointer });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
  // Per-frame values: cheap to push every render, never rebuild anything
  // (`state` rebuilds only when, spec-less and deprecated, it changes the pattern).
  useEffect(() => {
    handle.current?.update({ state, specState, inputs, voiceLevelInput, theme, paused, reducedMotion, label, onError, onFrame, labels, announce, rules });
  });

  // A box-layout pattern (edge `framing`, signal `playing`) fills the box it's given,
  // so it gets no square default; the app's `style` sizes it. Every other one is square.
  const box = useMemo(() => viewLayout({ spec, pattern, state }) === "box", [spec, pattern, state]);
  const sizing: CSSProperties = box ? { display: "block", width: "100%" } : { display: "block", width: "100%", aspectRatio: "1" };
  return <canvas ref={ref} className={className} style={{ ...sizing, ...style }} />;
}

export interface SinuaVoiceButtonProps {
  /** The voice source; give your `<SinuaView voice>` the same one. */
  source: VoiceSource | null;
  mode?: VoiceButtonMode;
  labels?: Partial<VoiceButtonLabels>;
  theme?: "auto" | "light" | "dark";
  className?: string;
  /** Size it here (default 56 x 56). */
  style?: CSSProperties;
  onChange?: (detail: VoiceButtonChangeDetail) => void;
}

/**
 * The voice button (docs/fx-view.md, *Voice button*): `<sinua-voice-button>`
 * under the hood, so the markup, keyboard and accessibility are the element's.
 *
 *   <SinuaVoiceButton source={source} mode="pushToTalk" />
 */
export function SinuaVoiceButton({ source, mode = "toggle", labels, theme = "auto", className, style, onChange }: SinuaVoiceButtonProps) {
  const ref = useRef<SinuaVoiceButtonElement>(null);
  defineSinuaVoiceButtonElement();
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    el.source = source;
    el.mode = mode;
    el.labels = labels ?? {};
    el.theme = theme;
  }, [source, mode, labels, theme]);
  useEffect(() => {
    const el = ref.current;
    if (!el || !onChange) return;
    const on = (e: Event) => onChange((e as CustomEvent<VoiceButtonChangeDetail>).detail);
    el.addEventListener("voicebuttonchange", on);
    return () => el.removeEventListener("voicebuttonchange", on);
  }, [onChange]);
  return createElement("sinua-voice-button", { ref, className, style });
}

// Helpers built on SinuaView: an avatar with the `talking` ring, and a voice message
// (`playing`) with drag-to-seek.
export { SinuaAvatar, AVATAR_INNER_RADIUS, type SinuaAvatarProps } from "./avatar.js";
export { SinuaVoiceMessage, voiceMessageSeek, type SinuaVoiceMessageProps } from "./voice-message.js";
