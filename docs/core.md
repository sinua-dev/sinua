# The `core` family

## Why this exists, and the direction that was chosen

`docs/gpt.md`'s taxonomy names `core` as "AI / system / processing /
intelligence". Two directions were on the table; the one **confirmed with
the user** is a small, **inline** shimmer / gradient-sweep indicator meant to
sit next to or inside other UI — a button, a line of text, a toolbar — the
way GitHub Copilot's, Claude's and ChatGPT's "generating…" indicators and
every SaaS skeleton-loading state do. The other candidate, a "nucleus" /
reactor-cluster visual, was considered and rejected for occupying the same
UI real estate as Orb (the same "too orb-adjacent" risk `signal`'s
`waveform` had to design around). `core` therefore lives in *secondary,
small-footprint* space, and its two states are the two most common inline
"something is being produced" cues.

It is the fifth and last sibling family (`crates/core_engine/src/core_fx/`
— the Rust module is `core_fx`, not `core`, because a user module named
`core` would shadow the `core` crate that std and the UniFFI/serde derive
macros expand paths into; the family is called `core` everywhere else).

## States and modes

| State | Mode | File | What it looks like |
|---|---|---|---|
| `generating` | `shimmer` | `modes/shimmer.rs` | A faint horizontal pill with a bright highlight gliding left to right and looping — the skeleton shimmer. Set `thickness` low for a VS-Code-style thin progress line. |
| `typing` | `dots` | `modes/dots.rs` | Three dots bouncing in sequence — the chat "typing…" indicator. |

No base-profile scaling (empty resolved opts, speed 1.0); periods are in
seconds.

## Prior art (fetched, not recalled, 2026-09-18)

- **react-loading-skeleton `skeleton.css`** (raw source): base `#ebebeb`,
  highlight `#f5f5f5`, `linear-gradient(90deg, base 0%, highlight 50%, base
  100%)` on an `::after` swept `translateX(-100%) → translateX(100%)`,
  `1.5s ease-in-out infinite`, disabled under `prefers-reduced-motion`.
  `shimmer` transcribes the sweep exactly.
- **VS Code `progressbar.css`** (raw source; the infinite bar Copilot's
  generating state shows): `.progress-bit { width: 2% }`, `translateX(0)
  scaleX(1) → 50%: translateX(2500%) scaleX(3) → translateX(4900%)
  scaleX(1)`, 4s linear (`steps(100)` for long-running). The thin variant of
  `shimmer` is this look.
- **androidx Material 3 linear indeterminate** (raw source):
  `LinearAnimationDuration 1750`, two lines with staggered head/tail
  (first 1000/1000ms, tail delayed 250ms; second 850/850ms, delayed
  650/900ms), `EasingEmphasizedAccelerate`. Read for the two-line idea;
  not transcribed — one highlight reads better inline.
- **shadcn/ui `skeleton.tsx`** (raw source): `animate-pulse rounded-md
  bg-accent` — the pulsing block variant (Tailwind pulse already lives in
  `beacon`'s `pulse`).
- **iMessage / Messenger typing indicator** (CodePen "CSS iMessage Typing
  Indicator", CodeFronts, via WebSearch): three dots, one `translateY`
  bounce, `ease-in-out`, delays 0 / 0.2 / 0.4s — "the whole trick is
  identical animation with offset start times". `dots` is exactly that.
- Perplexity's step spinner had no fetchable source and is out of scope.

## Geometry

**`shimmer`**: the pill is one `Polyline` from `x0 + thick/2` to `x1 −
thick/2` (round caps land exactly on `x0`/`x1`), `length` 0.84,
`thickness` 0.12, `trackOpacity` 0.18. The engine has no gradient, so the
highlight is `LAYERS = 9` concentric segments sharing one center, each
shorter than the one beneath and all at alpha 0.10 — source-over stacking
brightens the middle in steps (≈0.61 at the peak), a stepped bell that
reads as a glow at these sizes. Nine, not five: at five layers the segment
ends read as distinct nested pills (checked by rendering), at nine they
blur into one glow. Same thickness and center for every layer
means the round caps nest instead of bumping. The center sweeps from one
highlight-length before the track to one after it (`highlightLength` 0.35 of
the track) under CSS `ease-in-out` (`cubic-bezier(0.42, 0, 0.58, 1)`) over
`period` 1.5s, and layers are clipped to the track — so the glow enters
from off-track and leaves off-track, the CSS `-100%..100%` sweep.

**`dots`**: `dotCount` 3 at `spacing` 0.22, radius `dotSize` 0.06, each
rising `bounceAmplitude` 0.12·size on a half-sine (eased) during the first half
of `period` 1.2s and resting for the second, delayed by `k · delay` (0.2s);
alpha rises from 0.6 to 0.95 at the top.

Every mode-agnostic post-process applies (mute, interrupt flash; pointer
and audio pulse only affect frames with dots, so `typing` reacts and
`generating` doesn't).

## Tests

No golden vector. `shimmer`: the track spans exactly `length · size`
including caps and stays faint; the highlight is a stepped bell of
concentric, equally thick, progressively shorter layers; the highlight's
center moves monotonically left to right within a period, every layer stays
clipped to the track, and after the wrap only the on-track layers remain;
deterministic; the thin variant honors `thickness`. `dots`: three dots
evenly spaced and centered on one baseline; each dot's peak lands one
`delay` after the previous; a dot rises by exactly `bounceAmplitude · size`, is
brighter while rising, and rests on the baseline for its second half;
deterministic.

## Studio

The Web Studio's `families/core/{CoreStudio.tsx, knobs.ts}` — the same
minimal panel shape as Ring/Beacon (thumbnail state picker, reset links,
per-knob reset dots): shimmer period/thickness/highlight/track, dots
cycle/stagger/bounce, Color. Registered in `App.tsx`'s `FAMILIES` as
`core`.

## Known, not done

- **Real gradient: done as an option (`highlightFill` = 1, materials phase 1).**
  - The track is a solid pill `Fill` (a stadium polygon, 12 segments per cap).
  - The highlight is the **same pill** with a linear gradient band: transparent → peak → transparent, padded transparent beyond. That is react-loading-skeleton's `linear-gradient(90deg, base, highlight 50%, base)` swept across the track, clipped to it by the shape itself.
  - Peak alpha is `1 − 0.9⁹ ≈ 0.61`, the same as the stepped bell's.
  - The default stays 0 (the stepped layers) because the golden set and every renderer already draw that, and some renderers may not paint fills yet. See [`engine.md`](engine.md#paint-contract-fills-and-effects-materials-phase-1-2026-09-18).
- No `prefers-reduced-motion` equivalent in the engine — a caller that
  wants it freezes `t` (the same clock-is-the-caller's contract as `muted`).
- Native Studio panels: none, same as the other non-orb families.
