# Changelog

Every published package (`@sinua/core`, `@sinua/web`, `@sinua/voice` on npm;
`dev.sinua:sinua-*` on Maven Central; `sinua-swift`, `-livekit`, `-openai` for SwiftPM)
shares one version, from the root `VERSION` file. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/). A `-beta.N` version is a public prerelease: npm
tag `beta`, and it may still change incompatibly.

How to release: [`docs/publishing.md`](docs/publishing.md), *How to release*.

## Unreleased

### Added

- **Transcripts** (docs/audio-pipeline.md, *Transcripts*): live text for both speakers, for captions.
  - **API:**
    - `onTranscript` on a source;
    - `SharedVoiceSource.onTranscript` (Web) / `listenTranscript` (iOS, Android), which works before `connect()`;
    - `supportsTranscript` and `transcriptTiming`;
    - the shape is `{ role, text, final, turnId, truncated?, startMs?, endMs? }`.
  - **Modes:** synced to the played audio by default, or raw with `syncToAudio: false`. A barge-in ends the assistant's turn `truncated`, with only what was played.
  - **Sources:** `OpenAILiveVoiceSource` (GPT-Live) on Web, iOS and Android; `SimulatedVoiceSource` everywhere, including the React Native handle (`onTranscript`).
  - **Your own source:** `@sinua/voice` exports `TranscriptAssembler`; the rules live in `spec/transcript-cases.json`.
  - **Privacy:** display only. Sinua keeps nothing beyond the current turn and sends nothing anywhere.
  - **Measured on GPT-Live:** synced is within 250 ms for most words in Turkish (median 82 ms, p95 191 ms); English is looser (median ~190 ms, p95 ~515 ms). Raw text arrives ~650 ms ahead of the audio.

- **iOS: Apple privacy manifests.** Every Swift target (`sinua-swift`, `-livekit`, `-openai`)
  and the React Native pod ship a `PrivacyInfo.xcprivacy`, so an App Store upload no longer
  needs the app to declare Sinua's use for it (ITMS-91053). Declared: system boot time
  (`35F9.1`, `systemUptime` as the animation clock) and file timestamps (`C617.1`, Rust's
  standard library reading the app binary's own metadata). No tracking, no collected data.
  A CI check keeps the manifests in step with the sources and the engine's symbols. See
  [`docs/publishing.md`](docs/publishing.md), *Apple privacy manifests*.

### Changed

- **GPT-Live barge-in (`OpenAILiveVoiceSource`, Web / iOS / Android): the 1 s window now runs from
  the user's latest words, not their first.** Live sessions showed GPT-Live talking on for ~1 s after
  the user started, then switching straight to the new reply. The old rule missed those barge-ins,
  so `onInterrupt` never fired and the agent stayed `speaking` across replies. Now `onInterrupt` and
  the `speaking` → `listening` change come at such a cut-off. A short "mhm" under speech that goes on
  still doesn't count. `spec/openai-live-cases.json` has the case.

### Removed

1.14 takes out what the 1.14 review found unused or about to be replaced. None of it is
used by DevinFit (glowing + colour / gradient + GPT-Live), so DevinFit needs no change.

- `characterSlots`, `nearestSlot` and the `CharacterSlot` type (`@sinua/core`; `characterSlots`
  on iOS and Android). Migration: none; drag-to-dress comes back with the per-character
  wardrobe in a later version.
- `docs/character-svg-guides.md`. Migration: none; the SVG import was a Studio feature and is
  gone.
- **The liquid, particles and holographic materials.** An FX Spec file that uses
  `materials.liquid`, `materials.particles` or `materials.holographic`, or names `liquid` or
  `particles` in `performance.lowPower.disable`, is now **rejected** (an error naming the
  section), whatever its `fxSpec` version. Migration: delete the section. The typed
  components lose their `liquid`, `particles` and `holographic` props; `liquidSuitability`
  (`@sinua/core/dev`) and `particleDefaults` are gone; the `liquid*` / `particle*` / `holo*`
  keys do nothing as raw overrides. The low-power host default is now glow off only.
  An FX Spec 1.8 or 1.9 file with `states` drew particles in its voice states; it now
  resolves without them. The `liquid-orb`, `particles-orb` and `holo-orb` examples are gone.
- **The character wardrobe: cosmetics, loadouts and catalog packs.** An FX Spec file with a
  top-level `cosmetics` or `wardrobe` is now **rejected**, whatever its `fxSpec` version, and
  so is a recipe with `slots`, `tags`, `cosmetics` or a part's `role`. Gone with them:
  `applyLoadout`, `cosmeticsFor`, `frameStill`, `loadCatalog`, `unloadCatalog` and the
  `Loadout` / `LoadoutApplied` / `CosmeticFit` types (`@sinua/core`); `characterThumbnail`
  and `@sinua/web/catalog`; the views' `loadout` option (web, React, `<sinua-view>`, the
  typed character components); `SinuaLoadout`, `SinuaThumbnail`, `SinuaCatalog` and the
  `loadout` parameter (iOS, Android); `loadout`, `catalogs` and `@sinua/react-native/catalog`
  (React Native); `FxSpecPlayer`'s `loadout` option, `setLoadout` and `loadoutDiagnostics`;
  `StateTransition.wear()`, `wearing` and `WEAR_S`; `spec/catalog/catalog-1.json`,
  `spec/loadout-vectors.json` and `docs/character-cosmetics.md`. Migration: delete the keys
  and stop passing a loadout; a character-specific wardrobe is planned for a later version.
  The `party-hat` and `wardrobe-bean` examples are gone.
- **Named palettes.** `"palette": "sunset"` and `palette.theme` are now **rejected**, whatever
  the file's version, and `spec/palettes.json` is gone. Migration: write the colours out,
  by slot or by role (`{ "primary": "#F1774B", "dark": { "primary": "#DD562C" } }`); roles
  and the `dark` variant stay. The `themed-cuppa` example is gone.
- **Eye styles.** `params.eyeStyle` (and the typed components' `eyeStyle` prop and
  `EyeStyle` enum), a recipe's eye `style`, `iris` and `sclera`, the `iris` role and the
  built-ins' `iris` slot are gone; a file or recipe that still uses one is **rejected**.
  Migration: delete them; every character draws the shape eye it always drew by default.
  The `glossy-bean`, `pixel-beep` and `dot-hum` examples are gone.
- The built-in recipes lose their `tags`, `slots` and `iris` colour, and the
  `rich-bean`, `rich-buzzy`, `custom-character` and `remix-latte` examples their `slots`: the
  frames are unchanged (golden `sinua` byte-identical), but a file carrying one of these
  recipes resolves to a new registry key (`recipe:<id>:<hash>`).
- **The `concluding` pattern** (the crystallize mode: dots that snap into a crystal and let
  go). An FX Spec file whose `pattern` (base or a `states` entry) is `concluding` is now
  **rejected**, whatever its `fxSpec` version. Migration: pick another orb pattern
  (`progressing` or `confirming` read as "wrapping up"). The typed orb components lose
  `concluding` and the `driftAmplitude` / `lineWidth` props only it read. Golden `sinua`
  4.0.0 drops its 12 cases; every other case is unchanged.

## 0.1.0-beta.8

Size: web 511,877 B (gzip 382,107), Android arm64 .so 2,124,360 B, iOS .a 4,393,832 B (release builds).

### Breaking: Studio / dev APIs moved to `@sinua/core/dev`

The default `@sinua/core` entry now carries only what apps draw with; the dev tools moved to
a new entry, `@sinua/core/dev`. Change the import:

```ts
import { estimateCost, SimulatedVoiceSource } from "@sinua/core/dev"; // was "@sinua/core"
```

- Moved: `parameterCatalog`, `checkOverrides`, `estimateCost`, `fxSpecCost`,
  `liquidSuitability`, `conversationAt`, `conversationSampleNames`, `conversationSample`,
  `SimulatedVoiceSource`, and the JSON bridges `frameViaJson`, `frameWithOverridesViaJson`,
  `frameFromFxSpecViaJson`. Their types stay exported from `@sinua/core`.
- `@sinua/core/dev` re-exports the whole default API on the same (dev) wasm, so a tool that
  registers catalog packs or recipes can import everything from it and run one engine.
- iOS and Android: the published artefacts leave out the Swift / Kotlin functions only
  the Studios use: `estimateCost`, `fxSpecCost`, `liquidSuitability`,
  `parameterCatalogJson`, `checkOverrides` (the Android library is
  ~100 KB smaller installed, ~42 KB in the download). `packages/*/build.sh` keeps them for
  development (tests, the Studios); `SINUA_NATIVE_RELEASE=1` builds the published variant.
- Android install docs: leave JNA's `armeabi` / `mips` / `mips64` loaders out of your app
  (`packages/android/README.md`), so Play doesn't offer it to devices without the engine.
- The default bundle is about 48 KB smaller: the dev tools and the JSON frame bridges left
  it, and the transition and thumbnail frames now cross as packed arrays. Frames draw as fast
  as before.

### Added

- A new family, `character`: voice-assistant characters with a face (FX Spec 1.11,
  `docs/character.md`), as `SinuaCharacter` on every platform (`<sinua-character>` on the
  Web). The characters:
  - `buzzy`, a small space-hero assistant;
  - `hum`, a vintage studio microphone whose grille lights with the voice and whose tally
    light shows when it listens;
  - `wisp`, a helpful spirit whose sparkles gather, orbit and stream with the conversation;
  - `chirp`, a songbird that sings its answers: the beak opens with the voice, notes rise and
    the wings flutter.
  - The head turns like a solid (`turn`, ±25° by default, 0 = flat): the face slides round
    the head, the light stays put, near parts come forward. Idle looks corner to corner,
    listening turns to you, thinking looks up and away, speaking nods with the voice.
  - Shape eyes (no pupils, no brows); the gaze knows whose turn it is. It looks at you while
    listening, looks away while thinking, and blinks once at the end of your turn.
  - The mouth is a voice line, two even waves that flow; the agent's level sets their
    height (amplitude only). Effects are
    expressions: success, error, celebrate.
  - Options: `hue`, `mouth`, `accessories`, `look`, `turn`, `seed`. Mute squints and fades instead of
    greying out.
  - Drawn entirely with fills: no painter changed.
- FX Spec 1.11: `object: "character"`. A character takes `params.hue`, not `color` /
  `gradient`. 1.8–1.10 files resolve exactly as before
  (`spec/fx-spec-1.11-resolved.json`).
- FX Spec 1.12: `recipe`. A brand's own character travels in one FX Spec file: the file
  carries a character recipe and its `pattern` names the recipe's `id`. Every view draws it
  with no platform code (`spec/examples/custom-character.fxspec.json`). Recipe errors point
  into the recipe, and limits (64 KB, 48 parts, segments, counts, numbers) keep any file
  cheap to draw. Voice states come from a shared character language or a built-in's
  (`"profile": "chirp"`). 1.8–1.11 files resolve exactly as before
  (`spec/fx-spec-1.12-resolved.json`).
- Two new characters, made only from recipes: `cuppa`, a coffee mug on its saucer whose
  steam rises while it listens and thickens with its voice, and `bean`, a coffee bean on
  little feet with aroma sparkles. The new `steam` part draws soft wisps that rise and fade.
- A new character, `beep`: a small tin robot with arms. A hand goes to the ear to listen and to
  the chin to think, and the arms beat with the voice; the antenna light shows listening and
  thinking. The new `arms` part is a recipe part like the others, and `arms: false` turns the
  arms off.
- Expressions: a character takes `expression` (`happy`, `surprised`, `thoughtful`, `sad`,
  `sleepy`), as a view prop or an FX Spec 1.12 key in the base and in `states`. It shapes the
  eyes and the resting mouth (new "O" and frown mouths) while the voice state keeps the gaze and
  the talking mouth. Changes ease over 0.6 s.
- Recipe reference and schema: `spec/character-recipe-1.schema.json` describes every recipe key
  and part field (generated from the engine, so it can't drift), `docs/character-recipe.md` lists
  them, and `docs/character-remix.md` walks through a remix; the new example
  `remix-latte.fxspec.json` is Cuppa remixed into a latte glass. Example files point `$schema` at
  `https://sinua.dev/schema/fx-spec-1.json` (recipes: `…/character-recipe-1.json`).
- Remixing characters: `characterRecipe(id)` gives a built-in's recipe as JSON, a start for
  your own (`recipe` in an FX Spec 1.12 file). `@sinua/snippets` adds `fitPath` (any SVG path,
  arcs included, fitted into the recipe's 200-unit box) and `svgPaths`.
- Palettes: a character takes `palette`, a few of its colours by name
  (`{ body: "#E63946" }`), as a view prop or an FX Spec 1.12 key in the base and in `states`.
  Light and dark tones follow; a dark body lifts the eyes; `hue` never turns a given colour.
  `characterRecipe(id)` gives a built-in's recipe, whose palette names are the slots. Snippets print it as the `palette` prop.
- Tap to hop: a tap on a character plays the new `hop` effect. It hops, smiles, and glances
  toward the tap, then goes back to the voice state. It is on by default on
  `SinuaCharacter` (`tap: false` turns it off) and silent; on other objects `hop` does
  nothing. Apps can play it themselves with `trigger("hop")` / `SinuaEffect.HOP`.
- Character shapes from SVG paths: a recipe's body, patch or band can be a `path` (the
  commands `M L H V C S Q T Z`). Later subpaths are holes (a mug's handle). A concave body
  clips its inner layers exactly, and the outline keeps its width at sharp corners.
- `stateAge`, a runtime input every view now sets: seconds since the lifecycle state changed.
- State transitions: a catalog definition can mark a value as an *arrival*
  (`"transition": "arrive"`). A state change then takes the new state's value at once
  instead of interpolating it (a character's turn blink).

- FX Spec 1.13: `cosmetics`. A character can wear a hat, glasses or a badge, as data in the
  file: `body` and `eyes` parts on one of its slots (`headTop`, `face`, `neck`, `chest`), in
  the slot's units (`docs/character-cosmetics.md`). Cosmetics move with the pose, the hop and
  the head turn, take `palette` as `<id>.<name>`, and are left out at 20 px unless
  `accessories` is on. A cosmetic on `headTop` zooms the character out about its feet so the
  hat fits. `fits` and `fit` limit or nudge it per character; one a character can't wear
  warns and isn't drawn. A recipe may carry `cosmetics` too. The slots are public, with each
  built-in's `headTop.scale` set for hats (`docs/character-recipe.md`, *Slots and
  cosmetics*). Example: `spec/examples/party-hat.fxspec.json`. 1.8–1.12 files resolve
  exactly as before (`spec/fx-spec-1.13-resolved.json`).

- FX Spec 1.13: the richer look (`docs/character-recipe.md`, *The richer look*). Characters
  can be airbrushed, rim-lit and grained on today's painter, with no blur:
  - `shade` layers (soft masses clipped to the body: core shadows, bounce light, highlights, a
    head's shadow on the body, a blush that turns with the face), `rim` light, and `grain`;
  - elliptical radial lights (`[cx, cy, rx, ry, angle]`), stops with alpha, any number of stops,
    and `"light": "none"` overlay bodies;
  - new `grain` and `shading` props on `SinuaCharacter` (every platform) and FX Spec `params`;
    under low power an FX Spec sheds them for characters that have them;
  - the paint contract gains two values in existing fields: `FillGradient.kind` 2 (elliptical)
    and `Fill.blend` 2 (grain, a shared noise tile); the Web, iOS, Android and React Native
    painters and the Studio's SVG export draw them. The FFI record and the packed transport
    are unchanged.
  - Examples: `spec/examples/rich-bean.fxspec.json`, `rich-buzzy.fxspec.json`. The built-ins
    are unchanged.

- FX Spec 1.13: named palettes (`docs/character.md`, *Named palettes and roles*). One palette
  fits every character:
  - recipes map `primary` / `secondary` / `accent` to their slots (`roles`);
  - `palette` takes role names, a built-in palette (`"palette": "sunset"`; `sunset`, `ocean`,
    `forest`, `candy`, `mono`, `night`) and a `dark` variant that the views pick in a dark
    theme (the Web, iOS, Android and React Native views pass `dark`);
  - the `palette` prop on every view takes the same keys;
  - the Studio's character panel has a palette picker.

- FX Spec 1.13: eye styles (`docs/character.md`, *Eye styles*). Besides the shape eye:
  - `glossy`: a lens or white sclera, an iris and pupil that follow the gaze, highlights that
    stay with the light, a lid line;
  - `pixel`: the eye lit as a grid of glowing cells;
  - `dot`: a soft glowing point.
  - A recipe sets its own on its `eyes` / `faceScreen` part (`style`, `iris`, `sclera`); the
    new `eyeStyle` prop on `SinuaCharacter` (every platform) and `params.eyeStyle` override
    it. Blinks, the gaze and every expression work in each style; the built-ins keep the
    shape eye. The Studios have an Eye style picker.

- FX Spec 1.13: wardrobes and loadouts (`docs/character-cosmetics.md`, *Let end users pick*).
  - A file's `wardrobe` lists what an end user may pick (cosmetics, named palettes); a cosmetic
    may name its `category`.
  - A loadout (`{ "loadout": 1, "wear": [...], "palette": "...", "eyeStyle": "..." }`) is one
    small value the app stores; the new `loadout` option / prop on every view applies it and
    eases a change (a hat pops in, colours blend, a new eye style swaps in a blink). A stale
    loadout only warns.
  - For picker screens: `cosmeticsFor` (what fits, with a translatable reason), `frameStill`
    and the thumbnail helpers (`characterThumbnail` on the Web, `SinuaThumbnail` on iOS and
    Android), `applyLoadout`. `SinuaThumbnail.image` needs iOS 16 (SwiftUI's `ImageRenderer`);
    the package still supports iOS 15, where `frameStill`'s frame can be drawn yourself.
  - The Studio's character panel has a Wardrobe group (thumbnails, wear, *Copy loadout*).

- FX Spec 1.13: catalog packs (`docs/character-cosmetics.md`, *Catalog packs*). Sinua's free
  catalog (14 cosmetics, among them headphones that glow while speaking and a seasonal winter
  hat, 6 palettes and 5 eye colours) loads as data: `@sinua/web/catalog`, `SinuaCatalog` on iOS and Android,
  `SINUA_CATALOG` + the `catalogs` prop on React Native. Files name items as `"catalog:<id>"`;
  a brand loads its own pack (from a file or a URL) under its own namespace.
  - Fit by capability: a slot is enough (a bow tie needs a `neck`), and `requires` / recipe
    `tags` cover the rest; a new `frame` slot draws round the whole character.
  - A loadout entry may carry a bounded nudge (`offset`, `scale`, `rotate`).
  - The Studio's Wardrobe group lists the catalog beside the file's own items.

- FX Spec 1.13: the `silhouette` pattern (`docs/fx-spec.md`, *v1.13: silhouettes*), a head
  and shoulders of ~1,500 evenly spread dots (64 px; ~380 at 32 px, the outline alone at
  20 px) that lives with the voice: waves come in from the outline while it listens, dots
  flicker along the eye line while it thinks, and waves spread from the mouth with the
  agent's level while it speaks. The dots never change between states.
  - Two built-ins, `human` and `helmet`, or a file's own outline with its eye line and mouth
    (`"silhouette": { "path", "eyes", "mouth" }`).
  - `turnYaw` turns the head (the shoulders stay); `hologram` gives a hologram look (a bright
    rim, now and then a band sliding sideways), `scanlines` and `wire` add texture.
  - Medium cost; under low power (`performance.lowPower`) it draws the first 800 dots,
    spread just as evenly.

- Palette slots are named by the part they paint, never by colour: every
  character's main colour is `body`, and `outline`, `cheeks`, `shine` and `iris` mean the same
  everywhere (Buzzy's `shell` / `amber` / `cyan` are now `body` / `accent` / `glow`; the full
  table is in `docs/character.md`, *Palette*). `spec/character-slot-labels.json` gives every
  slot a label and a one-line description for colour pickers; the Studio shows them and keeps
  the light / dark tones behind *Advanced*.
  - Eye colour on every character: an `iris` slot and role (`palette: { "iris": "#7A4BD6" }`)
    colours the glossy eye; a loadout's `iris` picks one by name from `wardrobe.irises` or the
    catalog (`catalog:eyes-hazel`).

- Imported characters: a recipe part may name its `role` (`head`, `ears`,
  `arms`…), and a cosmetic's `behind` / `above` draws against it (a beanie behind a fox's
  ears). A recipe may now have 96 parts (was 48) and at most 4,096 path points in all.
  `docs/character-svg-guides.md` describes how to name an SVG's groups and guides so it becomes
  a character.
- A 1.13 file without `states` (or missing some) now reacts to the voice: each voice state
  takes the voice profile. 1.8–1.12 files resolve as before.
- Resolving a file with a large recipe again (a picker's thumbnails, an imported character)
  no longer re-parses the recipe: the engine finds it by its text, guarded by a second hash.
- Drag to dress: `characterSlots` gives a character's cosmetic slots where it
  draws them (frame units, following the pose), and `nearestSlot` picks the one a dropped item
  snaps to. Thumbnails of a recipe file are about 3× faster: whether a character is heavy to draw
  is now measured once per character, not on every resolve.

- Transitions (part 2: the views): every view (Web, iOS, Android, React
  Native) moves a weight per state on a continuous clock, so a change mid-transition
  heads somewhere else without a kink and three quick changes blend. Voice-state changes
  take the profile's time per pair; a file's own `curve` is kept, with the motion's
  velocity carried into it. The players pin their phase at speed changes (a state with its
  own speed no longer jumps the pose). From FX Spec 1.13 (and in views without a file),
  glowing and calibrating lay their dots out once for their densest state, so their
  sparser states look slightly more organic at rest; older files keep the old look.
- Android: JNA 5.19.1 (16 KB memory pages, required by Google Play for Android 15+); a CI
  check keeps every 64-bit `.so` 16 KB aligned, and the 32-bit libraries are aligned too.

- Transitions (part 1: the engine):
  - Rate keys (`pulsePeriod`, `period`, `surfaceSpeed`, …) can be accumulated by the view
    (`rates` on `transitionMix`, `<key>Cycles` overrides), so a state change late in a
    session no longer makes a pattern flash or boil. Plain frames are unchanged. This is a
    bug fix for every file, with no version gate.
  - `voiceBlend(sides, weights, target, size)`: the voice states' weighted mix, for views
    whose transition clock keeps a weight per state.
  - Counts that change between voice states fade instead of swapping: the orb lattice's
    dots, the spectrum's bars, sonar's echoes. Plain frames are unchanged.
  - Voice-state changes take the profile's time per pair when the file has no rule
    (0.25 s into speaking … 0.9 s back to idle; profileVersion 3). `FxTransition.authored`
    says whether the file wrote the curve.
  - The glow fades its faintest layers instead of dropping them, the speaker ring's
    shimmer grows from nothing, and a character's elbow eases where its two IK solutions
    tie (beep's resting arm moves by up to 4 units in the thinking pose).

- Voice (from DevinFit's device test):
  - iOS `OpenAILiveVoiceSource` / `OpenAIRealtimeVoiceSource` set up the audio session
    (`.videoChat` on the loudspeaker, Bluetooth; `audioSession: .receiver` / `.unmanaged`).
    Before, GPT-Live on iOS sent no mic audio and played nothing.
  - OpenAI sources on every platform leave `speaking` after an adaptive quiet tail: 0.7 s
    for a short reply, growing with the answer up to 1.7 s, so no pause inside an answer
    flips the state (it was ~300 ms); a barge-in stays instant.
  - Web: `headers` and `fetch` options on the OpenAI sources; a failed request throws with
    `status` and `body` (`FatalConnectError` / `VoiceHttpError`).

### Changed

- `parameterCatalog()` / `parameterCatalogJson()` no longer carry `description` texts: they are
  in the typed components' doc comments (your IDE shows them), in `spec/parameters.json` and on
  sinua.dev. The runtime embeds the catalog without them, minified (and the voice-state profile
  minified), about 10 KB less (gzip) in every app. Labels, ranges and defaults are unchanged.
- Characters are now **recipes** (`spec/characters/*.json`): a palette, a rig and a list of
  parts from the character part library, read by one schema-checked reader. The four
  characters draw exactly as before (golden byte-identical). A new look is a new recipe; a
  part can fade by voice state (`show`); each recipe declares slots for 1.13's cosmetics.
  Costs ~17 KB gzip in the wasm.

### Removed

- **SinuaEdge**: `SinuaEdge` / `<sinua-edge>` on every platform, FX Spec `object: "edge"` and
  its pattern `framing` (the in-app screen-edge glow, new in beta.6). **FX Spec files with
  `object: "edge"` now fail** with an error at `/object`, at every version: the one break of
  the "minor versions only add" rule, made once during the beta (docs/fx-spec.md,
  *Versioning*). Migration: remove the component or the file; Sinua has no replacement, so
  draw a screen-edge glow in the app itself. The box layout stays (`playing`, the voice
  message). Golden `sinua` 2.0.0 drops the 15 `framing` cases.

## 0.1.0-beta.7

### Added

- OpenAI GPT-Live (`gpt-live-1`): `OpenAILiveVoiceSource` on the Web (`@sinua/voice/openai`),
  iOS (`SinuaOpenAI`) and Android (`sinua-openai`).
  - Your server opens the session (`POST /v1/live/sessions` with its key); the source posts
    `{ "sdp": … }` to your `sessionUrl` and takes OpenAI's 201 JSON back unchanged (or the bare
    SDP). `sessionUrl` can't be `api.openai.com`, and a credential for it is never `sk-…`.
  - State without turn events: the model's audio is `speaking`, an open backend delegation is
    `thinking`, and user speech that stops the model is a barge-in. The same table on every
    platform: `spec/openai-live-cases.json`.
  - `disconnect()` closes gracefully (`session.close`, then waits for `session.closed`).
    `expired` / `connection_lost` reconnect with a new session.
  - `@sinua/voice/server`: `createOpenAILiveSession` and `openAILiveResponse` for your endpoint.
  - React Native doesn't have it yet. Beta: run against a live session on the Web; iOS and
    Android are tested against fakes and the shared table, and barge-in isn't tried live yet.
- iOS and Android typed components (`SinuaOrb`, `SinuaRing`, `SinuaSignal`, `SinuaBeacon`,
  `SinuaEdge`, `SinuaCore`) take `effect`, `labels`, `announce`, `haptics` and, on a spec,
  `rules`, like `SinuaView`. Before, only `SinuaView` could play a one-shot effect there. Web
  and React Native components already passed them through.
- `@sinua/snippets`: `effect` on `buildSnippets` and `effectLines(platform, effect)`, the
  lines that say how an app plays a one-shot effect (the Studio's Play effect adds them to its
  export); `@sinua/design`: a `sparkle` icon.
- WARP for OpenAI Realtime (`warp: true`, off by default): a pre-negotiated event channel plus
  `dcid`. On iOS and Android it also turns on libwebrtc's DTLS 1.3 / SNAP / SPED field trials.
  A `callsUrl` backend must forward `dcid`. On GPT-Live, `warp` sets only the native field
  trials (OpenAI documents no `dcid` there); experimental.

### Changed

- FX Spec 1.10: the shared voice-state profile no longer adds particles in listening /
  thinking / speaking, so views are calmer. Overrides or a state's `materials.particles` turn
  them back on. 1.8 and 1.9 files resolve exactly as before (the identity locks hold them);
  plain views follow the new profile. The orb `speaking` pattern sways within ±55° instead of
  turning edge-on.
- One-shot effects on box-layout patterns (the screen-edge `framing`, the voice-message bar
  `playing`) play in place: the rim or the bars turn green, red or gold and brighten, so a
  faint resting rim flashes too. There's no ring, tick or burst in the middle of the screen,
  and no shake that slides the rim away.
  `spec/effect-vectors.json` gains their cases.
- SimulatedVoiceSource: the user's turns are louder (0.40–0.78), so signal moves while listening.
- OpenAI Realtime: the credential rule now depends on where the credential goes. On OpenAI's
  own host (`api.openai.com`, the default) it must still be an `ek_…`. On your own calls
  endpoint (`callsURL` on iOS, `callsUrl` on Android and, new, on the Web) it's your own
  short-lived token in any shape, but never a raw `sk-…` key. This is the "sideband" setup,
  where your backend opens the OpenAI session with its own key and handles tools and
  transcripts server-side. Beta.6 refused any non-`ek_` token even there. React Native
  doesn't pass a calls URL yet.

## 0.1.0-beta.6

### Added

- Real state transitions on every platform. When a state change keeps the pattern (every
  voice state, most FX Spec states), the view interpolates the parameters and the speed,
  so the shape flows instead of two frames dissolving; the orb lattice trio (glowing /
  calibrating / progressing) morphs point by point; anything else cross-fades. Views
  without a spec now animate voice-state changes too (they used to jump). Engine:
  `transitionMix`, `frameTransitionWithOverrides`, `fxSpecTransition`; callers:
  `StateTransition` (Web, iOS, Android) and `FxSpecPlayer`.
- One-shot feedback effects on every platform (docs/fx-view.md, *One-shot effects*):
  `success` (a green pulse, a ring, a tick), `error` (a shake, a red tint) and
  `celebrate` (a particle burst), played once on top of any pattern.
  - Web: `trigger(name)` on the handle and on `<sinua-view>`. iOS/Android: the
    `effect: SinuaEffectTrigger?` parameter. React Native: `effect={{ name, key }}`.
  - Reduced motion keeps only the tint (and the tick in place).
  - Effects are spoken ("Done", "Something went wrong", "Well done") unless `announce`
    is false; `labels["effect:<name>"]` rewords them.
  - The engine draws them (`effectCode` / `effectAge` / `effectReduced` runtime keys,
    `effect_info`), so every platform shows the same frames (`spec/effect-vectors.json`).
- State-aware accessibility on every platform (docs/fx-view.md, *Accessibility*).
  - A view's accessible name follows the state it shows ("Coach, listening").
  - State changes are spoken, politely and rate-limited: a state is spoken after holding
    1 s, at most once per 3 s. Web uses an `aria-live` region, iOS VoiceOver
    announcements, Android TalkBack.
  - New view options: `labels` (per-state words, for translation), `announce` (default
    on), native `haptics` (a light tap on "listening", opt-in).
  - The engine owns the words and the timing (`a11y_accessible_name`,
    `a11y_announce_step`, `spec/a11y-announce-vectors.json`), so every platform says the
    same.
- FX Spec 1.9 `accessibility`: `{ name, states: { key: words }, announce }`.
- The views apply FX Spec 1.9 `rules`: without a voice bound, the state the rules derive
  from `inputs` wins over the app's `state`; `rules: false` turns it off.
  `fxSpecDeriveState` on the Web.
- Voice button on every platform: `<sinua-voice-button>` / React `SinuaVoiceButton`
  (`@sinua/web`), SwiftUI `SinuaVoiceButton`, Compose `SinuaVoiceButton` and React Native
  `<SinuaVoiceButton>`. It shows ready / connecting / listening / muted / error, derived
  from the source, around an engine-drawn ring that swells with the level.
  - Modes: toggle (connect, then mute / unmute) and push-to-talk. A long press or the ✕
    ends the session.
  - Accessible as a real button: the name follows the state, and there's an
    "End voice session" action.
  - One state table (`spec/voice-button-cases.json`) tests all platforms.
- `setMuted(muted)` on every built-in voice source: silence goes out, the session stays
  up. Also `onConnectionChange(connected)`, since an agent can be `idle` while
  connected. React Native: `handle.setMuted`, `onConnectionChange`, `onMuteChange`.
- `SharedVoiceSource`: one source, many listeners (a source holds one callback of each
  kind). Views bind a raw source through it, so a view and a voice button (or two views)
  share a source, and every view shows the source's mute.
- Simulated conversations: `SimulatedVoiceSource` on Web (`@sinua/core`), iOS, Android
  and React Native (`vendor: "simulated"`). A script of turns plays like a real agent
  (state changes, a speech-like level and bands, the barge-in cue) with no microphone,
  audio or network. The curves come from the engine (`conversationAt`), so every platform
  plays the same frames. Four built-in looping samples: `calendar`, `quick-answer`,
  `long-answer`, `barge-in`. Controls: play, pause, seek, loop, `onFrame` for captions.
  See docs/audio-pipeline.md, *Simulated conversations*.
- FX Spec 1.9: an optional `transitions` block sets each state change's duration and curve
  (`default`, `from->to`, `from->*`, `*->to`); default 0.6 s, easeInOut.
- Voice credentials, one contract on every platform (Web, iOS, Android, React Native).
  Every source takes `credentialUrl` (POSTs to your endpoint) or a `credential` provider,
  called again on every connect and reconnect, and reads one JSON shape:
  `{ credential, expiresAt?, url? }`. Gemini now resumes a dropped session with a fresh
  token, and ElevenLabs signs a new URL per connect. LiveKit also takes a credential URL
  or provider. Natively: `CredentialSource` (Swift `.url` / `.provider` / `.value`,
  Kotlin `url` / `provider` / `fixed`) and `init(credentialUrl:)` /
  `withCredentialUrl(…)`; React Native: `credentialUrl` / `credential` on every vendor.
- `@sinua/voice/server`: `mintOpenAIRealtimeCredential`, `mintGeminiLiveCredential`
  (model, voice and instructions locked into the token), `signElevenLabsUrl`,
  `mintLiveKitCredential` and `credentialResponse`. Server-only, no dependencies; runs on
  Node, Next.js, Workers, Deno and Supabase.
- `npx @sinua/voice dev-proxy`: serves those credentials on 127.0.0.1 from the keys in
  your `.env`, so you can try voice before you write a backend.
- `examples/voice-server/`: credential endpoint templates for Next.js, Express,
  Cloudflare Workers and Supabase Edge Functions.
- Web: `pointer: true` (`<SinuaView pointer />`, `<sinua-view pointer>`) turns on pointer and
  touch scatter: the dots near the pointer are pushed away from it, easing in and out. It
  was in the engine and the Studio, but no view fed it. Off by default; not applied under
  reduced motion or while paused. iOS and Android views follow; until then, pass
  `pointerX/Y/Radius/Strength` as overrides.
- FX Spec 1.9: an optional `rules` list derives the state from the app's inputs, e.g.
  `{ "when": { "input": "steps", "gte": 10000 }, "state": "goalReached" }`. Rules are
  tried in order and the first that holds wins; `hysteresis` keeps a state from
  flickering at its threshold. Engine: `fxSpecDeriveState(json, inputs, previous)`, a
  pure function (the caller keeps the previous state). Views call it next.
- Ring `talking`: a voice ring around an avatar ("who is speaking"). The ring thickens
  outward with the voice and never crosses the avatar; ripples travel out while
  speaking and in while listening, and an arc circles it while thinking. Parameters:
  `innerRadius`, `avatarGap`, `thickness`, `reach`, `idleOpacity`, `flow`, `rippleCount`,
  `shimmer`. The host view puts the image in the middle.
- Signal `playing`: a recorded voice message's waveform with its playback position,
  for chat bubbles. The app passes the clip's loudness as `envelope` (up to 64 values;
  Sinua decodes no audio) and the position as `progress`; bars before it are in full
  ink, the rest dimmed (`unplayedOpacity`). `barCount` resamples and keeps the peaks,
  and `playhead` draws the position line. It's the first box-layout pattern: it lays
  out in `size × aspect` by `size`, from the new `aspect` runtime input (catalog:
  `layout: "box"`; engine: `patternLayout(pattern)`). Every view passes its box's
  ratio and fills the whole box, so it fits a chat bubble.
- `SinuaEdge` (new object `edge`, pattern `framing`): an in-app screen-edge glow. A
  colourful rim hugs the inside of the screen or container (rounded corners,
  `cornerRadius`), grows inward with the voice and never leaves the box; colour flows
  round it (`hueSpread`, `flowSpeed`) and a bright segment circles it while thinking
  (`shimmer`). Hidden at rest, faint while listening. Box layout, portrait or
  landscape (`aspect` 0.125–8). Unlike the other objects it's colourful by default
  (`saturation` 0.7); 0 gives the grey ink.
- `SinuaAvatar` (React, SwiftUI, Compose): an image in a circle with the ring `talking`
  round it, for call grids, agent lists and chat headers. The image spans
  `2 × innerRadius` of the view, the same radius the ring draws round, so they can't
  drift apart; the ring is decorative and the image's label names the avatar.
- `SinuaVoiceMessage` (React, SwiftUI, Compose): signal `playing` with drag-to-seek. Pass
  `envelope` and `progress`; a drag or a tap reports the position under the finger
  through `onSeek`, on the same row of bars the engine draws
  (`playbackSeekProgress(aspect, x)`). It's a slider for assistive tech (arrow keys on
  the Web, adjustable on iOS, set-progress on Android).
- React `SinuaView` no longer forces a square (`aspectRatio: 1`) on a box-layout pattern;
  `viewLayout(options)` says which one a view is.
- `@sinua/core`: `OrbState` now lists `talking`, `playing` and `framing` (TypeScript callers
  got a type error for them), with a test that holds the union to the catalog;
  `FxSpec.object` and `ParameterObject.id` include `"edge"`; `ParameterPattern.layout`.
- `@sinua/snippets`: code for a box-layout pattern uses its own box (`framing` 180×390,
  `playing` 220×44; `snippetBox(pattern)`); square patterns are unchanged.
- Many small views at once:
  - on the Web, every view on a page shares one `requestAnimationFrame`;
  - a view whose short side is under 48 px / pt / dp draws at 30 fps by default (the
    app's `maxFps`, including 0 for display rate, or the spec's `performance.maxFps`,
    still wins);
  - on Android a view scrolled out of a clipping parent stops, as on the Web and iOS.
  Measured on the Web bench (`apps/fx-bench-web?grid=50&px=32`): 50 small orbs went
  from 869 to 459 ms of work a second, with no dropped frames. For a hundred or more,
  use `paused` (one still frame).

### Changed

- Views bind a raw voice source through `SharedVoiceSource` (each view keeps its own
  tracker), so a second view or a voice button on the same source no longer takes its
  callbacks. Subscribe on `SharedVoiceSource.of(source)` to listen yourself.
- Android: `VoiceSource` gains `onError` (it was on the vendor sources only), plus the
  optional `setMuted` / `supportsMute` / `onConnectionChange` / `reportsConnection`,
  all with defaults.
- State changes default to 0.6 s easeInOut (were a 250 ms cubic ease-out cross-fade on
  spec views, and a jump on plain ones). The views' `crossFade` option is now optional and
  overrides every change's duration when set (`0` = a cut, as before); Swift/Kotlin
  `crossFade: Double? = nil`.
- Voice: session config belongs to the server. `OpenAIRealtimeVoiceSource`'s `voice` and
  `instructions` are ignored (they only ever applied to the removed client-side mint);
  `GeminiLiveVoiceSource`'s `instructions` is deprecated on every platform (a locked
  token wins). OpenAI's `getCredential` (Web, React Native) is a deprecated alias of
  `credential: provider`.
- Orb `progressing`: the day/night line is fixed on screen. The lit side grows from the
  left as `progress` rises, and the sphere keeps spinning under it. It used to turn with
  the sphere, which read as a spinning two-tone ball, not progress.
- Beacon `scanning`: `blipCount` defaults to 0 (was 3). Blips are the targets your app
  found (e.g. nearby devices), so pass the count; an empty scope reads as "still looking".
  Raising the count adds a blip and moves none.
- Beacon `scanning`: with many range rings (`ringCount` 6–8) the scope no longer turns into a
  jumbled grid. The dots shrink to fit the gap between rings, so `dotSize` is a maximum. The
  default 4 rings look the same.

### Removed

- **Breaking (voice, all platforms):** `allowInsecureApiKey` and the raw-API-key paths
  behind it: OpenAI's on-device/in-browser `client_secrets` mint, and Gemini's `?key=` /
  `x-goog-api-key` socket. A raw key is now always refused before the mic opens. Use
  `npx @sinua/voice dev-proxy` locally and `@sinua/voice/server` in your backend. Also
  gone: iOS/Android `OpenAIRealtimeSignaling.clientSecretRequest` / `clientSecret`, and
  `OpenAIRealtimeVoiceSource`'s native `model`/`voice`/`instructions` parameters (the
  backend sets them; Swift keeps a deprecated overload that ignores them).
  `InsecureCredential.Refused` is replaced by `CredentialError` (Swift) /
  `CredentialException` (Kotlin).

### Fixed

- React Native: a spec view's state changes follow the spec's `transitions` (the native
  side always passed a 0.25 s `crossFade`), and a plain view's `state` reaches the native
  view, so `<SinuaView pattern state="listening">` gets the voice-state behaviour. The
  view also takes `audioStrength` for the bound voice's pulse.
- Parameter catalog: `ink`'s `specPath` is `/ink`, the top-level key FX Spec 1.8 reads; it
  said `/params/ink`, which the resolver rejects as an unknown key. A test now writes every
  pattern's every parameter at its `specPath` and resolves the file.
- Web: a view that can't be seen (scrolled off screen, or in a background tab) no longer
  draws a frame on every `update()`. An app that feeds a live level into such a view
  painted each update anyway. The view now draws the latest state when it becomes
  visible again. A paused, visible view still draws its still frame at once.

## 0.1.0-beta.5

### Fixed

- Android: `sinua-core` now ships consumer R8/ProGuard rules (JNA and the UniFFI bindings).
  In an app built with R8 (a minified release build), the first engine call threw
  `UnsatisfiedLinkError: Can't obtain peer field ID for class com.sun.jna.Pointer`: a
  plain app crashed on its first SinuaView. Apps no longer need their own rules for Sinua.

No change on iOS or the Web: npm and SwiftPM move to beta.5 in lockstep.

## 0.1.0-beta.4

### Fixed

- Android: `SinuaView`'s frame loop now runs as an infinite animation
  (`withInfiniteAnimationFrameNanos`), so an app's Compose UI tests can go idle while a
  view animates on screen. Before, `ComposeTestRule.waitForIdle` failed with
  `ComposeNotIdleException` on any screen showing a SinuaView. Drawing is unchanged.

No change on iOS or the Web: npm and SwiftPM move to beta.4 in lockstep.

## 0.1.0-beta.3

### Changed

- iOS: the xcframework's static libraries no longer carry debug info (`strip -S` in
  `packages/ios/build.sh`). The release zip drops from 23 MB to about 4 MB; the size an app
  gains is unchanged (checked byte for byte against beta.2).

### Added

- CONTRIBUTING.md (maintainers only for now), SECURITY.md (private vulnerability
  reporting), CODEOWNERS.

No runtime or API change from beta.2.

## 0.1.0-beta.2

### Added

- Android: `dev.sinua:sinua-core`, `sinua-view` and the voice modules on Maven Central,
  signed. The `dev.sinua` namespace is verified.

### Fixed

- Release: a blank signing key (an unset CI secret) now stages unsigned instead of failing.
- The publishing docs: npm gives a package's first version `latest` whatever `--tag` says.

No runtime or API change from beta.1: npm and SwiftPM move to beta.2 in lockstep.

## 0.1.0-beta.1

The first public beta: npm (`@sinua/core`, `@sinua/web`, `@sinua/voice`, `beta` tag) and
SwiftPM (`sinua-swift`, `-livekit`, `-openai`). The Android packages (`dev.sinua:sinua-*`
on Maven Central) follow with `0.1.0-beta.2`, once the `dev.sinua` namespace is verified.

### Added

- The runtime: one Rust engine (`core_engine`) drawn on every platform. On the web it is
  WebAssembly: `@sinua/core`, and `@sinua/web` with `<sinua-view>` and typed components.
  On iOS it is the SwiftUI view `SinuaView` in the `Sinua` product. On Android it is
  `sinua-view` for Compose.
- FX Spec 1.8: the file format the Studio exports and every runtime reads. It has 34
  patterns, and each supports four voice states (idle, listening, thinking, speaking).
  1.8 is the oldest version the runtime reads. A key added in a later minor is an error
  in a file that declares an older one, and is dropped rather than silently applied.
- `glowing`: `depthTone` (0..1, default 1). At 0 every dot has the same tone; the sphere
  keeps its depth through dot size and opacity. At 1 it draws exactly as before.
- Voice: `@sinua/voice`, `SinuaVoice` and `sinua-core` drive the four states from an
  audio source. The adapters for Gemini Live, ElevenLabs, LiveKit and OpenAI Realtime are
  beta. On iOS the voice types are a separate product, so an app that only draws doesn't
  link microphone code.
- Typed components generated from the parameter catalog on every platform, with the
  public API frozen by a snapshot.

### Deprecated

- `@sinua/core`'s three JSON-bridge frame functions, each naming its direct twin.
