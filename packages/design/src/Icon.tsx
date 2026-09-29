import type { ReactNode, SVGProps } from "react";

export type IconName =
  | "play"
  | "pause"
  | "pointer"
  | "mic"
  | "tone"
  | "image"
  | "vector"
  | "sun"
  | "moon"
  | "copy"
  | "check"
  | "reset"
  | "chevron"
  | "sliders"
  | "transition"
  | "close"
  | "bolt"
  | "bell"
  | "download"
  | "mute"
  | "cloud"
  | "fade"
  | "loop"
  | "gauge"
  | "sparkle";

// Hand-drawn 24-unit strokes (Figma's UI3 drew its own set rather than
// pulling a library, for the same reason: a dozen consistent glyphs beat
// a dependency). All stroke-based at 1.75 so they read at 14-16px.
const GLYPHS: Record<IconName, ReactNode> = {
  play: <path d="M8 5.5v13l10-6.5z" fill="currentColor" stroke="none" />,
  pause: (
    <>
      <rect x="6.5" y="5.5" width="4" height="13" rx="1.2" fill="currentColor" stroke="none" />
      <rect x="13.5" y="5.5" width="4" height="13" rx="1.2" fill="currentColor" stroke="none" />
    </>
  ),
  pointer: <path d="M6 4.5l12.5 7.2-5.4 1.7-2.4 5.4z" />,
  mic: (
    <>
      <rect x="9" y="3.5" width="6" height="11" rx="3" />
      <path d="M5.5 11.5a6.5 6.5 0 0 0 13 0M12 18v2.5M9 20.5h6" />
    </>
  ),
  tone: <path d="M3 12h2.5l2-5 3 10 3-14 3 12 2-3H21" />,
  image: (
    <>
      <rect x="4" y="5" width="16" height="14" rx="2" />
      <circle cx="9" cy="10" r="1.5" />
      <path d="M20 15.5l-4.5-4.5L8 18.5" />
    </>
  ),
  vector: (
    <>
      <path d="M6 16.5C6 10 10 6 16.5 6" />
      <rect x="4" y="16" width="4" height="4" rx="1" />
      <rect x="16" y="4" width="4" height="4" rx="1" />
    </>
  ),
  sun: (
    <>
      <circle cx="12" cy="12" r="4" />
      <path d="M12 3v2M12 19v2M3 12h2M19 12h2M5.6 5.6l1.4 1.4M17 17l1.4 1.4M18.4 5.6L17 7M7 17l-1.4 1.4" />
    </>
  ),
  moon: <path d="M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z" />,
  copy: (
    <>
      <rect x="9" y="9" width="11" height="11" rx="2" />
      <path d="M5 15V6a2 2 0 0 1 2-2h9" />
    </>
  ),
  check: <path d="M5 12.5l4.5 4.5L19 7.5" />,
  reset: (
    <>
      <path d="M4 12a8 8 0 1 0 2.3-5.7" />
      <path d="M4 4v5h5" />
    </>
  ),
  chevron: <path d="M6 9l6 6 6-6" />,
  sliders: (
    <>
      <path d="M4 7h9M17.5 7H20M4 17h3.5M12.5 17H20" />
      <circle cx="15" cy="7" r="2" />
      <circle cx="10" cy="17" r="2" />
    </>
  ),
  transition: (
    <>
      <circle cx="7" cy="12" r="3.5" />
      <circle cx="17" cy="12" r="3.5" strokeDasharray="2.2 2.2" />
      <path d="M10.5 12h3" />
    </>
  ),
  close: <path d="M6 6l12 12M18 6L6 18" />,
  // Added by the families session for the Interrupt (barge-in flash) button.
  cloud: <path d="M7 18.5a4.5 4.5 0 0 1-.6-8.96A6 6 0 0 1 18 8.5a4 4 0 0 1 .5 8H7z" />,
  bolt: <path d="M13 3L5 13.5h6L10 21l8-10.5h-6z" strokeLinejoin="round" />,
  // A four-point sparkle -- the Studio's celebrate effect (the native Studios' sparkles / AutoAwesome).
  sparkle: <path d="M12 3.5l2 6.5 6.5 2-6.5 2-2 6.5-2-6.5-6.5-2 6.5-2z" strokeLinejoin="round" />,
  // An arrow into a tray -- the export drawer's Download (the .fxspec.json).
  download: <path d="M12 4v11M7.5 10.5 12 15l4.5-4.5M5 19h14" strokeLinejoin="round" />,
  // A ringing bell -- Beacon's Trigger (the native Studios' bell / NotificationsActive).
  bell: <path d="M6 16V11a6 6 0 0 1 12 0v5l1.5 2h-15zM10 20.5a2 2 0 0 0 4 0M3.5 8.5A9 9 0 0 1 6 4M20.5 8.5A9 9 0 0 0 18 4" strokeLinejoin="round" />,
  // A circular arrow -- a looping auto-demo.
  // A speedometer: the frame-rate cap.
  gauge: <path d="M4 17a8 8 0 1 1 16 0M12 17l4.5-5" />,
  loop: <path d="M19 12a7 7 0 1 1-2.05-4.95M19 4.5V8h-3.5" />,
  // Three dots losing opacity left to right -- the Fade out trigger.
  fade: (
    <>
      <circle cx="5.5" cy="12" r="2.75" fill="currentColor" stroke="none" />
      <circle cx="12" cy="12" r="2.75" fill="currentColor" stroke="none" opacity="0.55" />
      <circle cx="18.5" cy="12" r="2.75" fill="currentColor" stroke="none" opacity="0.2" />
    </>
  ),
  mute: (
    <>
      <path d="M15 9.5V6.5a3 3 0 0 0-6 0v3M9 12a3 3 0 0 0 5.2 2M5.5 11.5a6.5 6.5 0 0 0 10.4 5.2M18.5 11.5a6.4 6.4 0 0 1-.9 3.3M12 18v2.5M9 20.5h6" />
      <path d="M4 4l16 16" />
    </>
  ),
};

export function Icon({ name, size = 16, ...rest }: { name: IconName; size?: number } & SVGProps<SVGSVGElement>) {
  return (
    <svg
      width={size}
      height={size}
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={1.75}
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden="true"
      focusable="false"
      {...rest}
    >
      {GLYPHS[name]}
    </svg>
  );
}
