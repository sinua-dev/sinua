// `<SinuaView/>` -- a thin React wrapper over `mount` (React is an optional
// peer of @sinua/web; the root entry never imports it).
//
//   import { SinuaView } from "@sinua/web/react";
//   <SinuaView spec={mySpec} voice={source} style={{ width: 160, height: 160 }} />
import { useEffect, useRef, type CSSProperties } from "react";
import { mount, type FxHandle, type SinuaViewOptions } from "./mount.js";

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
  const { spec, pattern, state, size, overrides, speed, voice, voiceOptions, specState, inputs, voiceLevelInput, crossFade, theme, paused, reducedMotion, label, onError, maxFps, lowPower, onFrame } =
    options;
  const deps = [spec, pattern, size, overrides, speed, voice, voiceOptions, crossFade, maxFps, lowPower];
  useEffect(() => {
    if (first.current) {
      first.current = false;
      return;
    }
    handle.current?.update({ spec, pattern, size, overrides, speed, voice, voiceOptions, crossFade, maxFps, lowPower });
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
  // Per-frame values: cheap to push every render, never rebuild anything
  // (`state` rebuilds only when, spec-less and deprecated, it changes the pattern).
  useEffect(() => {
    handle.current?.update({ state, specState, inputs, voiceLevelInput, theme, paused, reducedMotion, label, onError, onFrame });
  });

  return <canvas ref={ref} className={className} style={{ display: "block", width: "100%", aspectRatio: "1", ...style }} />;
}
