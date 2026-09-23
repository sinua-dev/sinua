# Effects research: math & shapes for a second family

This is a research dump, not a decision record — nothing here is scheduled.
It exists to make the next "should we add a second `crates/core_engine`
family" conversation start from a survey instead of from scratch. Read
[`engine.md`](engine.md#other-families-considered) first: `border-beam`,
`liquid-gooey`, and `metal-fx` were already evaluated and deferred with
specific reasons (mostly: no upstream golden vectors to prove correctness
against, or low Rust-sharing value). Anything proposed here should be
checked against that same bar — real-time on constrained mobile hardware,
fits (or deliberately extends) the existing `Dot`/`Line` grayscale-ink paint
contract (`spec/orbs-spec.json`, `orbs/core.rs`), and ideally has some
independent way to prove correctness rather than relying on code review.

Two research passes below, done independently and merged into this file.

## A. Mathematical / physical techniques

Upstream check first, since `engine.md` flags it as worth re-verifying each time: as of 2026-09-16, `Jakubantalik/Libraries.dev` (and the older `Jakubantalik/Libraries` mirror) still ships no `spec/`, `ports/`, or golden-vector infrastructure for `liquid-gooey` or `metal-fx` — only `packages/thinking-orbs` and `packages/border-beam` have that (`scripts/extract-golden.ts`, `spec/*-spec.json`, `ports/ios`, `ports/react-native`). `liquid-gooey`'s package is just `design/`, `src/`, `README.md`; `metal-fx` has a `demo/` and a WebGL `src/` but nothing resembling a frozen test vector. The deferral reasoning in `engine.md#other-families-considered` still holds unchanged — a `liquid-gooey` port today would still have zero independent correctness proof.

Every technique below is judged against the same four questions the task asks: what it is, why it fits the AI/voice-agent state use case (the original plan, section 2: reactive spheres/waveforms that make a model's internal state — listening/thinking/speaking — legible at a glance), real-time cost at 24-600 primitives/frame on mobile, and whether a golden-vector or closed-form proof is realistic — because that's the bar `orbs` already clears (`spec/orbs-golden.json`, `testing.md`) and any new family is implicitly held to it.

### Curl noise / divergence-free vector fields

Curl noise takes the gradient of a scalar potential field and rotates it 90 degrees to get a velocity field with zero divergence — particles advected by it swirl and interleave without ever clumping or fountaining apart, because there are no sources or sinks. This is exactly the "Curl Noise (Girdap Alanları)" line in the original plan's section 3A that never got built. For a voice-agent "thinking"/"searching" state it would look like smoke or eddying fluid rather than the orbits/globe modes' rigid rotation — a genuinely different motion signature from anything `orbs` currently has, which is worth something given all 9 existing modes share one `Proj` and one `vnoise`. Cost: computing curl of a value-noise potential needs 4 noise evaluations per point per axis (finite-difference gradient) instead of `vnoise`'s 1, so roughly 4-8x the noise cost per dot — at 600 dots that's still trivially cheap (a few thousand `hash_d` calls, not fluid-sim scale), the concern would only bite if ported to a much denser field. Correctness proof: hard. There is no widely-agreed reference implementation the way Fibonacci lattices or Lissajous curves have closed forms — curl noise's exact output is a function of whichever noise basis backs it (here it'd be `vnoise`'s own hash, not real Perlin), so a "golden vector" would only be provable against *this repo's own* frozen output, not an independent source. That's strictly weaker than what `orbs` has (upstream-vendored vectors) but comparable to a from-scratch mode with no upstream at all — still better than nothing, since a self-generated frozen fixture at least catches drift/regressions even without external provenance.

### Verlet integration for cloth/spring systems

Verlet integration stores position and previous-position (no explicit velocity) and updates via `x_new = 2x - x_prev + a*dt^2`; it's popular for cloth and rope sims because distance constraints (two points must stay `d` apart) can be satisfied by direct position correction after the integration step, with no stiffness-driven instability the way explicit spring-force integration has. This is the other named-but-unbuilt item from the original plan ("Verlet Entegrasyonu: yaylanma, kumaş esnemesi"). For voice-agent states it maps naturally to a "connecting"/"weaving" upgrade: a soft mesh of dots linked by constraint edges that visibly flexes and settles, which would read as more organic than `web`'s current static constellation-wiring or `braid`'s fixed strand math. Cost: a constraint-relaxation pass is O(constraints) per substep, typically 2-8 substeps/frame for stability; at orbs' scale (a `web` preset tops out around a few dozen nodes and edges, well under 600) this is negligible — cloth sims with thousands of particles run at 60fps on a phone, so tens of nodes is not a real constraint. Correctness proof: realistic. Verlet + distance constraints is deterministic given fixed initial conditions, a fixed integration order, and a fixed substep count — a hand-built reference (Rust or even a spreadsheet) can be independently re-derived from the textbook update rule, which is a genuine closed-form-adjacent proof even without an upstream vendor. This is actually a *stronger* correctness story than curl noise, because the algorithm itself (not just a specific implementation's hash function) is the spec.

### Quaternion-based rotation

Quaternions represent 3D rotation as a 4-component unit hypercomplex number, avoiding gimbal lock and enabling `slerp` (spherical linear interpolation) between orientations — the third named-but-unbuilt item of the original plan. `engine.md` is explicit that this was never a real need: `Proj`'s spin+tilt+orthographic projection has exactly 2 rotational degrees of freedom (yaw, tilt) and gimbal lock only bites with 3+ combined into a single Euler chain. Quaternions would only earn their keep if a new family needed genuinely free 3D orientation (e.g. a mode that tumbles independently on all 3 axes rather than orbiting a fixed viewing frame) or needed to interpolate smoothly *between* two arbitrary orientations (e.g. crossfading from a "listening" orientation to a "speaking" one without visible axis-snapping). That's a real distinguishing use case worth flagging even though nothing in the current 9 modes needs it. Cost: negligible — quaternion multiply/normalize is a handful of flops per point, same order as `Proj::project`'s current trig chain. Correctness proof: strong. Quaternion rotation composition and slerp have well-known closed-form definitions independent of any vendor (Shoemake's 1985 slerp formula, standard quaternion-to-matrix conversions) — a golden vector set could be generated from first principles or cross-checked against any standard graphics math library, which is actually a *better*-provenance situation than porting upstream's own bespoke trig, since it's checkable against public literature rather than one author's implementation choices.

### Metaball / marching-squares silhouette blending (what `liquid-gooey` actually is)

A metaball field sums each blob's influence as an inverse-distance (or Gaussian) falloff into a scalar field, then extracts an isosurface — in 2D that's marching squares, walking a grid and linearly interpolating where the field crosses a threshold to build a smooth outline. This is the real mechanism behind upstream's `liquid-gooey` package (per the web check above: `LiquidItem.tsx`/`imageMelt.tsx`, silhouette-layer-plus-crisp-DOM-on-top). It's the most visually distinctive candidate on this whole list for an AI-state UI — blobs that stretch, merge, and separate read unambiguously as "liquid identity," which is a strong metaphor for an agent's attention gathering and dispersing (e.g. multiple "thought" blobs coalescing while `solving`, or splitting while `listening` picks up multiple speakers). It is also the one technique here that **cannot** be expressed as the existing `Dot`/`Line` paint contract at all — marching squares outputs a closed polyline (or, for a filled look, a polygon), not discrete dots/thin lines, so it would require a new primitive (e.g. a `Path`/`Polygon` with a fill rule) and a renderer capable of filling it, which none of the current platform canvases need today (`architecture.md` notes only Web has a real renderer; iOS/Android/RN have none at all yet). That's a bigger structural ask than any other technique surveyed. Cost: for N blobs sampled on a G×G grid, it's O(G² × N) field evaluations plus O(G²) marching-squares lookups; keeping G small (e.g. 32-64 per axis) and N in the single digits keeps this well within mobile budget, but it is categorically higher than 600 point transforms — this is the one technique here where "primitive count" isn't the right cost model at all, grid resolution is. Correctness proof: exactly the problem `engine.md` already identified — upstream's `liquid-gooey` has no `spec/`/`ports/`/golden files (reconfirmed above), so a port has no independent vendor-provided proof. However, marching squares itself is closed-form and textbook (a specific lookup table plus linear interpolation), and metaball field summation is a stated formula — so a *from-scratch* implementation, unlike curl noise, could be proven correct against the algorithm definition itself rather than needing upstream's blessing. That distinction matters: this is a case where "no upstream golden vectors" (the stated deferral reason) doesn't necessarily mean "no independent proof is possible," just that *this project's own convention* of vendoring upstream's vectors specifically wouldn't apply — worth revisiting the framing if this family is picked up.

### Flocking / boids

Boids (Reynolds 1987) is three local rules — separation, alignment, cohesion — applied per-agent against nearby neighbors, producing emergent flock/swarm motion with no central choreography. For an AI-state visualization this reads as "many small things with individual, faintly autonomous behavior converging into one coherent whole" — a distinctive metaphor for cohering (`solving`, `composing`) or dispersing (`searching`) that's different in character from `orbs`' current fixed-formula placement (Fibonacci lattice positions, orbital paths) because boids' emergent paths are not prescribed in closed form, they fall out of the simulation. Cost: naive boids is O(N²) neighbor checks per frame; at N≤600 that's 360,000 pair checks worst case, which is still cheap in Rust with no allocation per check (plain float math over a flat array), but it's the first technique on this list where the algorithm's complexity class is actually worse than linear in N — a spatial grid/bucket would flatten it back to near-O(N) if this ever needed to scale up, though at these counts it's optional. Correctness proof: weak-to-moderate. Boids has a canonical rule description but no single "reference" numeric output the way a closed-form curve does — order of neighbor iteration, tie-breaking, and rule weighting all affect the exact trajectory, so a golden-vector suite would (like curl noise) only be provable as internally consistent, not against an external ground truth. It is at least fully deterministic given fixed neighbor-iteration order, so a locally-generated fixture is realistic even without an external one.

### Lissajous curves

A Lissajous curve is the closed-form parametric plot `x = A sin(a*t + delta), y = B sin(b*t)` — trivial to compute, and its exact shape is fully determined by the frequency ratio `a:b` and phase `delta`. This is arguably the single best correctness-proof candidate on this entire list: the curve family is a two-line formula with centuries of prior art, so a golden-vector table can be hand-derived and independently checked with a calculator, not just re-run against the same code. For a voice-agent "listening" state specifically, Lissajous figures are a natural fit for an audio-reactive waveform display — `wave` mode already rolls a waveform through concentric rings, and a Lissajous-driven variant (frequency ratio modulated by pitch/amplitude input, per the original plan's section 2C "normalized numeric arrays" input contract) would give a literal X/Y phase-plot read on two audio channels or two frequency bands, which is a recognizable "oscilloscope" visual language that reads as immediately technical/live in a way `orbs`' current curves don't attempt. Cost: trivial — two sin evaluations per point, cheaper than `vnoise`'s hash chain. This is close to a "free" addition to an existing mode rather than a new family: it could plausibly be a `wave` sub-mode rather than a whole new module.

### The Gielis superformula

The superformula is a single parametrized equation (Gielis, 2003) generalizing circles, polygons, and a huge range of star/flower-like closed curves via `r(phi) = (|cos(m*phi/4)/a|^n2 + |sin(m*phi/4)/b|^n3)^(-1/n1)`. It's notable because `shaping`/`morph` already does almost exactly this job by hand — cycling a dotted outline through circle → triangle → square via bespoke per-shape math (`modes/morph.rs`) — and the superformula would let a single parametrized function sweep continuously through a much wider shape space (organic petal/star forms, not just three hardcoded polygons) by animating `m`/`n1`/`n2`/`n3` over time, which fits `morph`'s existing "shaping" narrative better than adding more hardcoded shape cases. Cost: one formula evaluation per angle sample, same order as `morph`'s current per-vertex math — negligible. Correctness proof: strong and realistic — it's a single well-known published closed-form equation, so a golden table is derivable directly from the paper's formula with a calculator, independent of any implementation, similar in strength to the Lissajous case. This is a good candidate for extending `orbs::modes::morph` rather than a new family.

### Reaction-diffusion patterns

Reaction-diffusion (Gray-Scott, or the classic Turing model) simulates two chemical concentrations diffusing and reacting on a grid, producing organic spot/stripe/coral patterns from simple local rules iterated many times. It's visually striking but a poor fit here for several compounding reasons: it fundamentally wants a dense 2D grid (thousands of cells) evolved over hundreds of iterations to reach a stable pattern, which is a completely different cost model from 24-600 discrete primitives — either the grid gets downsampled to something coarse enough to render as ~600 dots (losing what makes the patterns interesting) or it needs a raster output, which (like metaballs) isn't expressible in the `Dot`/`Line` contract at all. It's also a bad fit for the "AI/voice-agent state" use case specifically: reaction-diffusion patterns are static/slowly-settling textures, not something that reads instantly as a live, responsive state change the way a spin rate or wave amplitude does — settling takes seconds to minutes of simulated time, which doesn't map to snappy state transitions. Correctness proof: feasible in principle (Gray-Scott's update rule is published and deterministic on a fixed grid+seed+iteration count) but not a strong reason to pick this technique given the fit problems above. Net: interesting math, weak match for this engine's actual constraints.

### L-systems

Lindenmayer systems generate structure by iteratively rewriting a string of symbols against production rules, then interpreting the final string as turtle-graphics draw commands (typically for branching/fractal patterns like plants). This has a genuinely good fit for a `Line`-only mode: the turtle-graphics interpretation step naturally emits line segments, exactly `orbs`' existing `Line` primitive (currently only `web` mode uses lines at all), so an L-system "growth" state could plausibly render as a branching structure extending outward — a strong metaphor for `weaving`/`connecting`/`composing` as literal growth rather than the current braid/wiring/ribbon math. Cost: string rewriting for a handful of iterations produces a bounded, predictable symbol count (choose iteration depth to cap segment count near the 24-600 budget directly), and turtle interpretation is O(symbols) with only a running position/heading state — cheap. Correctness proof: strong. Given a fixed axiom, production rules, iteration count, and turtle step/angle, the output string and resulting segment list are fully deterministic and can be hand-traced or computed independently of any particular Rust implementation — this is one of the more provable techniques here, on par with Verlet's textbook-derivable status.

### Voronoi / Delaunay tessellation

A Voronoi diagram partitions a plane into regions closest to each of a set of seed points; its dual graph is the Delaunay triangulation. `web` mode already draws a constellation of lines between points, which is conceptually adjacent — Delaunay edges would give a similarly "connected" look but with a mathematically principled (rather than heuristically chosen) edge set, which could be a legitimate `web` variant: "these dots are wired according to proximity structure" rather than whatever nearest-K heuristic `web.rs` uses today. It's a plausible fit for `connecting`, since a Delaunay mesh reads as "everything reachable from everything, structured" in a way that fits an agent forming connections. Cost: this is the one technique here with real algorithmic complexity to worry about — a naive Delaunay is O(N²) or worse; a proper incremental/divide-and-conquer implementation is O(N log N), which is standard but nontrivial to implement correctly from scratch (it's a classically fiddly piece of computational geometry, prone to degenerate-case bugs — collinear points, cocircular points). At N in the dozens (a `web`-scale preset) this is a non-issue computationally, but it's the most implementation-risk-heavy geometry on this list. Correctness proof: realistic and strong — Delaunay triangulation of a fixed point set has one correct answer (up to well-understood degenerate-case tie-breaking), so a small fixed point-set golden test is derivable by hand or cross-checked against any standard computational-geometry library (e.g. compare against `scipy.spatial.Delaunay` or CGAL output offline) independent of this codebase — arguably a *better*-provenance situation than anything upstream's own trig-heavy modes offer, since it's checkable against math, not against one author's implementation.

### Real Perlin / Simplex / Worley noise (vs. the current hash-based value noise)

`orbs::core::vnoise` is value noise: hash the lattice corners, smoothstep-interpolate between them. It is deliberately *not* Perlin or Simplex noise, and the original plan's "Perlin/Simplex noise" prediction in section 1 was wrong about the actual upstream engine (`engine.md` calls this out directly). Value noise has a known visual signature — a faint axis-aligned "grid" bias and blobbier, less isotropic detail — because it interpolates *values* at lattice points rather than *gradients*. Real Perlin/Simplex noise (interpolating pseudorandom gradient vectors instead of scalar values) looks more isotropic and organic, and Worley/cellular noise (distance to nearest of a set of feature points) produces a completely different cell/facet visual signature useful for a distinct "faceted" look. This isn't a new family so much as a drop-in upgrade path: any mode currently calling `vnoise` (used for surface jitter/perturbation across several modes) could swap in a Perlin implementation with the same call signature, and it would change texture quality without changing any mode's structural logic. Cost: classic Perlin is a few more operations per call than value noise (gradient dot-products instead of raw hashed values) but is still O(1) per sample and just as allocation-free — negligible at this dot/line scale. Correctness proof: this is the most literature-grounded option on the entire list — Ken Perlin's improved noise (2002) has a published reference implementation, and Simplex noise likewise has canonical reference code from Perlin himself; a golden-vector table could be generated directly from the canonical public-domain reference implementation, giving *external, non-upstream* provenance that's arguably stronger than what `orbs` has today (which is proven against one author's implementation, not a public standard). If a "second family" or `orbs` extension wants the single lowest-risk, highest-confidence-of-correctness improvement on this list, this is it — small in scope, but it's the one change here that's almost purely upside against the project's own provenance bar.

### Summary judgment against the project's stated bar

Ranked by how well each clears the "independent correctness proof" and "fits Dot/Line without new primitives" bars together: Lissajous curves and the Gielis superformula are the strongest on both counts (closed-form, published, fit as `wave`/`morph` extensions rather than new families) and the lowest-effort wins. Real gradient noise (Perlin/Simplex) is the best pure quality-upgrade with the best provenance story, again with no new primitives needed. Verlet+constraints, quaternions, and L-systems are all textbook-derivable (strong proof) and fit the existing point/line contract, making them the most defensible *new-family* candidates if the project wants one. Curl noise and boids are visually appealing but only self-consistently provable, not externally — acceptable but strictly weaker than `orbs`' current vendored-golden-vector standard. Metaballs (the real `liquid-gooey` mechanism) and reaction-diffusion are the two techniques that need a new fill-capable primitive/renderer the platforms don't have yet (`architecture.md` notes only Web has a real renderer today) — metaballs is at least algorithmically provable from first principles despite upstream having no golden vectors of its own, which nuances (without overturning) `engine.md`'s stated deferral reason; reaction-diffusion is a weak fit for this project on visual-metaphor and cost-model grounds independent of the proof question. Voronoi/Delaunay sits in between: strong proof story, fits the existing `Line` contract, but carries the most from-scratch implementation risk of anything surveyed.

## B. Shapes & visual concepts

### What other products use, briefly

A quick survey (Siri/Apple Intelligence, ChatGPT voice mode, Gemini, Alexa,
Copilot, plus a handful of open-source "audio-reactive orb" projects —
`voiceorb`, `particula`, `audio-reactive-lab`'s "Pulse", `SoundVisualizer`,
`react-ai-voice-visualizer`) turns up a small, recurring set of primitives:
continuous waveforms/spectrum bars, blob/metaball morphs (usually a fragment
shader with Perlin-displaced surface and a fresnel rim), one-shot ring
pulses, particle bursts, and starfields. Two things stand out as relevant to
`orbs` specifically:

- **Alexa's light ring communicates state almost entirely through color**
  (blue = listening, yellow = notification, green = call, red = muted,
  spinning orange = setup), not geometry — a channel `orbs` doesn't have at
  all (`Dot.white` is luminance, not hue). Every current mode has to encode
  state in *motion and silhouette* alone, which is a real constraint worth
  keeping in mind when judging "is this new mode actually distinct" — two
  modes that only differ in a color a grayscale ink can't express would
  collapse together on this engine.
- **Microsoft's Copilot avatar moved from a 3D gradient "pearly blob" toward
  more literal/anthropomorphic treatments** (reporting in late 2025 mentions
  an experiment with faces). That's a divergent design direction from
  `orbs`' abstract-geometry approach and not something to chase here — noted
  only because it confirms the anthropomorphic end of this design space is
  already being explored elsewhere, so staying abstract/geometric remains
  `orbs`' actual differentiator.

None of ChatGPT/Gemini/Siri's specific looks (soft-edged pulsing gradient
circle, amplitude-driven scale/glow) are reproducible in a grayscale
dot/line vocabulary without a filled/gradient primitive — see the family
concepts below rather than the mode proposals for that territory.

### Upstream check: Libraries.dev / `thinking-orbs` since the last review

Two things changed upstream since `engine.md`'s "other families considered"
section was written:

- **`Libraries.dev` gained a fifth package, `img-fx`** (`packages/img-fx` —
  confirmed via the GitHub contents API, not just the README blurb). It's a
  WebGL "loading mosaic that periodically reveals an image" effect for
  wrapping cards (three presets: two pixel-mosaic shaders and a diagonal
  gradient sweep with per-cell flicker). It's a card-loading effect, not an
  agent-state orb, and it's WebGL/`three`-dependent like `metal-fx` — same
  low Rust-sharing verdict as `metal-fx` in `engine.md` would apply. Worth
  knowing about, not worth re-evaluating as a family candidate.
- **`Jakubantalik/thinking-orbs` (the `orbs` source repo) has an open,
  unmerged, third-party PR** ("Add fluid cosmic orb states", PR #1, not
  authored by Jakub Antalik) proposing four new states — `cosmic`, `nebula`,
  `liquid`, `nova` — described as "fluid, colorful cosmic thinking-orb
  states," still Canvas-2D-only but explicitly breaking the monochrome
  contract (the PR's own `COSMIC_ORBS.md` flags this: "Cosmic modes bypass
  the grayscale `paint()` path and draw color directly"). Mechanically:
  radial-gradient glow lobes drifting over a dark void wash, a Fibonacci-
  lattice starfield reused from the existing engine, a glass rim stroke, and
  (for `liquid`) a polar-harmonic silhouette `R(θ) = R₀(1 + ΣAₙ·trig(nθ ±
  ωₙt))` instead of a circular clip. **This is unmerged and not vendored
  anywhere** — no golden vectors exist for it, it isn't in the published
  `thinking-orbs` npm package as of this check, and it isn't something to
  port. It's cited here only as external signal (from the same upstream
  author's ecosystem) that "color + gradient blob, still cheap, still no
  WebGL" is a direction being explored one repo over — which lines up
  independently with the `Spectra` family concept below, arrived at from
  the product survey rather than from this PR.

### New `orbs` modes (fit the existing `Dot`/`Line` grayscale vocabulary)

#### Sonar ping — one-shot acknowledgment

All 9 existing modes are continuous, indefinite loops — there is no mode for
a *transient* event. A sonar ping is a single expanding ring of dots (or two,
staggered) that brightens, grows from a point near the center to the sphere's
silhouette radius, and fades — done in well under a second, not looped.
Achievable entirely with existing `Dot` fields (radius of the ring = f(t),
alpha fades to 0 as it reaches the edge) and `finalize_frame`'s existing
alpha-cull. **State it communicates**: "received" / "task complete" / "sent"
— a receipt acknowledgment, distinct in kind (not just in geometry) from
every current mode because it's the only one that would need a defined *end*
rather than a phase that loops.

#### Spectrum bars — discrete audio-reactive ring

A ring of fixed angular dot-columns (like `wave`'s rings, but angularly
discretized instead of a continuous rolling waveform), each column's dot
count/height jumping to an independent value: the classic circular-equalizer
look. Where `wave` reads as smooth and continuous (good for "listening" —
receiving a steady stream), spectrum bars read as percussive and discrete —
good for a `speaking` state, which the current 9 states don't actually have
(`listening` covers input; nothing covers "the agent is talking right now").
Visually distinct from `wave` the way a bar-graph EQ is distinct from an
oscilloscope trace, even though both are "audio visualizations."

#### Warp starfield — depth-motion tunnel

Every current mode keeps dots on (or just off) a fixed-radius shell, moving
tangentially; none of them move a dot's *depth* toward/away from the viewer
over its lifetime. A warp/hyperspace field respawns dots near the center
with small `r` and streaks them outward in z, growing brighter and larger as
they approach the silhouette edge, then recycling — the classic
"warp-speed"/loading-tunnel look. **State**: `initializing`/`waking` (cold
start, first response of a session) — reads as "spinning up," distinct from
the other 9 because it's the only mode with genuine radial depth throughput
rather than shell-surface motion.

#### Chladni bloom — nodal standing-wave clustering

Distribute dots on the sphere (Fibonacci lattice, already available via
`fib_dir`) and modulate each dot's alpha/radius by a spherical standing-wave
function (sum of a few low-order terms, cheaply — no need for real spherical
harmonics, `vnoise`'s existing hash-noise machinery could seed pseudo-mode
numbers) that slowly changes its mode number over time. Instead of `web`'s
organic, continuously-drifting graph or `globe`'s uniform field, dots
visibly clump into shifting symmetric nodal bands and empty gaps — a
kaleidoscope-like snap between discrete symmetric states rather than smooth
drift. **State**: a `calibrating`/`tuning` state (matching pitch/tempo to a
speaker) — cymatics is a real physical metaphor for "settling into
resonance with an input," which is conceptually different from `connecting`
(building a graph) or `solving` (mechanical scramble/unscramble).

#### Eclipse terminator — determinate progress

A day/night terminator line sweeps across the sphere and dots on the dark
side are dimmed (not removed) while the bright side stays lit — differs
from `globe`'s scan (a thin meridian sweep over a constant field) by being a
full hemisphere split whose terminator angle can be driven by an actual
**percentage**, not just elapsed time. This is the one proposal here that
is functionally new, not just visually new: every existing mode is
indeterminate ("something is happening"), and this one is the first
candidate for a determinate ("N% done") state — useful for e.g. a model
download, a long tool call with known duration, or upload progress. Needs
no new primitive, just an opts key (`progress: 0..1`) threaded through
instead of `t`.

#### Crystallize — polyhedron wireframe snap

Dots drift loosely (borrow `web`'s value-noise drift) and then snap into
the exact vertex positions of a wireframe icosahedron (or octahedron),
edges drawn as `Line`s, hold briefly at full rigidity, then release back to
drift — repeat with a different polyhedron each cycle. Reads as "settling
into certainty": loose/organic to rigid/symmetric is a different kind of
transition than anything in the current set (`morph` interpolates between
*outlines*, not between disorder and a rigid 3D solid; `rubik` twists but
never leaves its rigid grid). **State**: `concluding`/`confirmed` — the
crystalline solid reads as "answer locked in," a distinct emotional register
from `shaping`'s continuous cycling between outlines.

### Shape considered and rejected as redundant: DNA double helix

A two-strand double helix (vs. `braid`'s three strands) was an obvious
candidate from the "what recurs elsewhere" list, but it's not proposed here:
it's mechanically the same idea as `braid.rs` with `strandN` set to 2 instead
of 3 plus a fixed (non-plaiting) phase offset — not a new visual family, just
a parameter change on an existing mode. If a "2-strand" look is ever wanted,
it belongs in `braid`'s preset/opts space (`spec/orbs-spec.json`), not as a
new mode file.

### New family concept: `spectra` — colored gradient-blob orb

The one thing every real product surveyed above has that `orbs` structurally
can't do is color. `spectra` would be a second family built around a filled,
gradient-shaded blob silhouette instead of a dot cloud: a soft radial or
conic gradient fill, an audio-reactive amplitude driving overall scale/glow
intensity, and a low-order polar-harmonic wobble on the silhouette edge (the
same `R(θ) = R₀(1 + ΣAₙ sin/cos(nθ ± ωₙt))` trick the unmerged upstream PR
above uses, and one that `liquid-gooey`'s deferred write-up already flagged
as using spring physics for something similar). **New primitives needed**:
a filled path/silhouette type (not just point clouds), a gradient fill
(multi-stop, radial or conic — `Dot.white` is a single luminance scalar,
nowhere near enough), and a hue/saturation channel somewhere in the paint
contract. **What it communicates**: warmth, liveliness, brand personality,
and — critically — states that are emotionally/valence-coded rather than
activity-coded: success (warm glow), error/caution (a sharper color shift),
celebratory moments — none of which a grayscale dot cloud can express no
matter how the geometry changes. This is the highest-value second-family
candidate of anything considered (here or in `engine.md`) specifically
*because* it's the one axis (color) the current family cannot cover by
adding more modes, only by adding a primitive.

### New family concept: `threadtrail` — particle motion trails

A dot cloud where each particle also renders a short, fading polyline of its
own last N positions (a comet trail), alpha falling off along the trail's
length — distinct from `orbits`' "ghost paths," which are static rings
showing the *orbit shape*, not a per-particle motion history tied to actual
velocity. **New primitive needed**: a `Trail` (an ordered, alpha-ramped
point list per particle, or additive-blended short polyline) — `Line` today
is a single static two-point segment with fixed endpoints per frame, not a
decaying history buffer. **What it communicates**: throughput/flow — data
moving somewhere, tokens streaming out, a file transferring — a `generating`
or `transferring` state where the *rate and direction* of motion is the
message, which none of the current 9 modes make legible (their motion reads
as "alive," not as "carrying something from A to B").

### New family concept: `glyphmorph` — filled iconography instead of abstract geometry

An extension of `morph`'s existing arc-length path-blend machinery
(`modes/morph.rs` already interpolates between closed paths and lays dots
evenly along the blend), but morphing between literal small icon silhouettes
(magnifying glass, gear, waveform glyph, checkmark) instead of
circle/triangle/square, and filling the interpolated path solid instead of
dotting its outline. **New primitive needed**: filled-path rendering (the
morph "blend two closed paths" math already exists and would transfer
directly — the gap is purely "fill" vs. "dot the outline"). **What it
communicates**: literal, accessible state signaling — a checkmark silhouette
unambiguously means "done" in a way no abstract geometric shape does,
useful anywhere the abstract vocabulary needs a fallback for
comprehension (accessibility, first-run onboarding, non-native audiences).
The tradeoff against `orbs`' current abstract style is real (icons read as
"a status icon," not "a living thing thinking") — this is offered as a
concept to weigh, not a clear win, unlike `spectra` above.
