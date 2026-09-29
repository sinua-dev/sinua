// `<SinuaVoiceMessage/>`: a recorded voice message's waveform (signal `playing`) with
// drag-to-seek -- the chat-bubble control.
//
//   import { SinuaVoiceMessage } from "@sinua/web/react";
//   <SinuaVoiceMessage envelope={peaks} progress={t / duration} onSeek={(p) => audio.currentTime = p * duration}
//     style={{ width: 220, height: 40 }} />
//
// Sinua decodes no audio: pass the clip's loudness as `envelope` (up to 64 values, 0..1)
// and the position as `progress`. A drag or a tap reports the position under the finger
// through `onSeek`, on the same row of bars the engine draws (`playbackSeekProgress`).
// Keyboard: it's a slider (arrow keys move 5 %, Home / End jump).
import { useMemo, useRef, type CSSProperties, type KeyboardEvent, type PointerEvent } from "react";
import { playbackSeekProgress } from "@sinua/core";
import { SinuaView, type SinuaViewProps } from "./react.js";

export interface SinuaVoiceMessageProps extends Omit<SinuaViewProps, "spec" | "pattern" | "specState" | "label" | "style" | "className"> {
  /** The clip's loudness, oldest first, 0..1 (up to 64 values; resample longer clips). */
  envelope: number[];
  /** The playback position, 0..1. */
  progress: number;
  /** The position the user dragged or tapped to (0..1), while dragging and on release. */
  onSeek?: (progress: number) => void;
  /** Accessible name. Default "Voice message". */
  label?: string;
  className?: string;
  /** Size it: a wide box, e.g. `{ width: 220, height: 40 }`. */
  style?: CSSProperties;
}

/** The position under a point `x` px from the left of a `width` x `height` box. */
export function voiceMessageSeek(width: number, height: number, x: number): number {
  if (!(width > 0 && height > 0)) return 0;
  return playbackSeekProgress(width / height, x / height);
}

export const MAX_ENVELOPE = 64;

export function SinuaVoiceMessage({ envelope, progress, onSeek, label = "Voice message", overrides, className, style, ...view }: SinuaVoiceMessageProps) {
  const dragging = useRef(false);
  const keys = useMemo(() => {
    const o: Record<string, number> = { ...overrides, progress: Math.min(1, Math.max(0, progress)) };
    envelope.slice(0, MAX_ENVELOPE).forEach((v, i) => (o[`envelope${i}`] = v));
    return o;
  }, [overrides, envelope, progress]);
  const seekAt = (e: PointerEvent<HTMLDivElement>) => {
    const r = e.currentTarget.getBoundingClientRect();
    onSeek?.(voiceMessageSeek(r.width, r.height, e.clientX - r.left));
  };
  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const step = { ArrowLeft: -0.05, ArrowDown: -0.05, ArrowRight: 0.05, ArrowUp: 0.05 }[e.key];
    const to = e.key === "Home" ? 0 : e.key === "End" ? 1 : step != null ? progress + step : null;
    if (to == null || !onSeek) return;
    e.preventDefault();
    onSeek(Math.min(1, Math.max(0, to)));
  };
  return (
    <div
      className={className}
      role="slider"
      tabIndex={0}
      aria-label={label}
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={Math.round(Math.min(1, Math.max(0, progress)) * 100)}
      onKeyDown={onKey}
      onPointerDown={(e) => {
        if (!onSeek) return;
        dragging.current = true;
        e.currentTarget.setPointerCapture?.(e.pointerId);
        seekAt(e);
      }}
      onPointerMove={(e) => dragging.current && seekAt(e)}
      onPointerUp={(e) => {
        if (dragging.current) seekAt(e);
        dragging.current = false;
      }}
      onPointerCancel={() => (dragging.current = false)}
      style={{ position: "relative", width: "100%", height: 40, touchAction: "pan-y", cursor: onSeek ? "pointer" : undefined, ...style }}
    >
      <SinuaView {...view} pattern="playing" overrides={keys} label="" style={{ position: "absolute", inset: 0, width: "100%", height: "100%" }} />
    </div>
  );
}
