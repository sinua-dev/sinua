// `<SinuaAvatar/>`: an image in a circle with the ring `talking` round it -- the
// "who is speaking" indicator for call grids, agent lists and chat headers.
//
//   import { SinuaAvatar } from "@sinua/web/react";
//   <SinuaAvatar src={user.photo} alt={user.name} voice={source} style={{ width: 56 }} />
//
// The image's diameter is `2 × innerRadius` of the box -- the same `innerRadius` the
// ring draws round, so the two can't drift apart. The ring is decorative; the image's
// `alt` names the avatar.
import { useMemo, type CSSProperties } from "react";
import { SinuaView, type SinuaViewProps } from "./react.js";

export interface SinuaAvatarProps extends Omit<SinuaViewProps, "spec" | "pattern" | "specState" | "label" | "style" | "className"> {
  /** The image URL. */
  src: string;
  /** The avatar's accessible name (the ring is decorative). */
  alt: string;
  /** The image radius as a fraction of the box (the ring's `innerRadius`). Default 0.34. */
  innerRadius?: number;
  className?: string;
  /** Size it with `width` (the box stays square). */
  style?: CSSProperties;
}

/** Default image radius: the ring `talking`'s own `innerRadius` default. */
export const AVATAR_INNER_RADIUS = 0.34;

export function SinuaAvatar({ src, alt, innerRadius = AVATAR_INNER_RADIUS, overrides, className, style, ...view }: SinuaAvatarProps) {
  const r = Math.min(0.44, Math.max(0.1, innerRadius));
  const ring = useMemo(() => ({ ...overrides, innerRadius: r }), [overrides, r]);
  const inset = `${(50 - r * 100).toFixed(3)}%`;
  const diameter = `${(r * 200).toFixed(3)}%`;
  return (
    <div className={className} style={{ position: "relative", width: "100%", aspectRatio: "1", ...style }}>
      <img
        src={src}
        alt={alt}
        style={{ position: "absolute", left: inset, top: inset, width: diameter, height: diameter, borderRadius: "50%", objectFit: "cover" }}
      />
      <SinuaView {...view} pattern="talking" overrides={ring} label="" style={{ position: "absolute", inset: 0, width: "100%", height: "100%" }} />
    </div>
  );
}
