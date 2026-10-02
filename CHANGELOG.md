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
  (`{ shell: "#E63946" }`), as a view prop or an FX Spec 1.12 key in the base and in `states`.
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
