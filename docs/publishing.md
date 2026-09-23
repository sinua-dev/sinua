# Publishing: how to release, and what a consumer gets

Nothing is published yet. The pipeline is built and dry-run end to end (2026-09-21): one
command produces every artefact a release would publish, and each was installed and
consumed from outside the repository. What's left needs the accounts
and their secrets, nothing else.

**The names are decided and filled in** (2026-09-22): `@sinua/*` on npm and
`dev.sinua:sinua-*` on Maven, from the public repository `https://github.com/sinua-dev/sinua`
(the manifests' `repository`, `homepage` and `bugs`, the POM's URL/SCM/developer defaults
in `gradle/publish.gradle.kts`); the SwiftPM package `Sinua` from
`https://github.com/sinua-dev/sinua-swift`; the pod `SinuaCore`. The install snippets
(`apps/site/snippets/install/`) name those URLs, and their versions are kept at `VERSION` by
`set-version.mjs`. The packages stay `private: true` until release, which is what makes
`npm-pack.mjs` pack from a scratch copy.

## How to release

**One version.** The root `VERSION` file (`0.1.0-beta.1` now) is the only place a version
is chosen. `scripts/release/set-version.mjs` writes it into the five `package.json`
files, the exact `@sinua/core` peer of `@sinua/web` and `@sinua/voice` (they release in
lockstep), `sinua.version` in `packages/android/gradle.properties` and the coordinates in
`packages/android/README.md`. `--check` names every copy that disagrees; CI's web job and
`scripts/ci-local.sh` run it, so a hand-edited version fails there and not at release.

```sh
node scripts/release/set-version.mjs 0.1.0-beta.2   # bump: VERSION + every copy
# add the entry to CHANGELOG.md, commit, then:
git tag v0.1.0-beta.2 && git push origin v0.1.0-beta.2
```

A version with a `-` suffix is a **prerelease**: npm dist-tag `beta`, a GitHub
prerelease. It never *moves* npm's `latest`, with one exception npm itself makes: a
package's very first publish also gets `latest`, whatever `--tag` says. That is why
`0.1.0-beta.1` is `latest` too (2026-09-23); the first stable release moves it.

**What the tag does** (`.github/workflows/release.yml`). The `version` job fails unless
the tag is exactly `v` + `VERSION`. Then three jobs, one per registry, each on its own
runner:

| Job | Builds | Publishes, only when its secrets exist |
|---|---|---|
| `npm` | core (wasm), web, voice; `npm-pack.mjs --publishable` | `npm publish` core → web → voice, `--access public --tag beta\|latest`. Provenance from the public repository (npm refuses it from a private one) |
| `maven` | the `.so` files and bindings; every module staged to a Maven-layout directory, **signed** | the staging directory zipped and uploaded to the Central Portal (`publishingType=AUTOMATIC`) |
| `swift` | the xcframework, zipped (`ditto`, deterministic), `swift package compute-checksum`, the three distribution trees | each tree pushed to its `sinua-dev` repository and tagged; a GitHub release per repository, the zip attached to `sinua-swift`'s |

A missing secret skips that job's publish step with a notice, so a half-set-up account
never half-publishes silently. Every job uploads its artefacts to the run.
A manual run (**Actions → Release → Run workflow**) is a dry run: it builds and uploads
the artefacts and publishes nothing.

| Secret | For |
|---|---|
| `NPM_TOKEN` | an automation token of the `sinua` npm org |
| `CENTRAL_USERNAME`, `CENTRAL_PASSWORD` | a Central Portal user token |
| `SIGNING_KEY`, `SIGNING_PASSWORD` | the ASCII-armoured PGP key Maven Central requires (its public half on a keyserver) |
| `SWIFT_DIST_TOKEN` | write access to `sinua-dev/sinua-swift`, `-livekit`, `-openai` |

**The same build, locally.** `scripts/release/build.sh` writes everything to
`out/release/<version>/`: `npm/` (three tarballs), `maven/` (the staging repository),
`swift/` (the zip, `checksum.txt`, the three trees) and `manifest.json` (every file with
its sha256). `--dry-run` adds `swift-local/`: the same trees with the binary target on
a copy of the local zip and the vendor packages on the sibling tree by path (the only
differences from the published form), so a scratch app can build against them.
`--only npm|maven|swift` builds one target, which is how release.yml calls it. It needs
the engine builds first (`packages/core` `npm run build`, `packages/android/build.sh`,
`packages/ios/build.sh`), like CI.

**Why three SwiftPM repositories.** SwiftPM reads `Package.swift` from a repository's
root, and ours live in `packages/ios*`. It also fetches every dependency a package
declares, so an app that only draws must not download LiveKit or WebRTC.
`scripts/release/swift-dist.mjs` generates each tree from the monorepo (sources,
licences, README, the manifest without test targets). Nothing is hand-copied, and the
distribution repositories are never edited directly:

| Repository | Products | Depends on |
|---|---|---|
| `sinua-swift` | `Sinua`, `SinuaVoice`, `SinuaGeminiLive`, `SinuaElevenLabs` | the zip on its own release (`binaryTarget(url:checksum:)`) |
| `sinua-swift-livekit` | `SinuaLiveKit` | `sinua-swift` at exactly this version, LiveKit's SDK |
| `sinua-swift-openai` | `SinuaOpenAI` | `sinua-swift` at exactly this version, WebRTC |

### Dry run, 2026-09-21 (`0.1.0-beta.1`)

`build.sh --dry-run` from the working tree, 266 files, then each target consumed from
outside the repository:

- **npm:** core 288 KB / 22 files, web 40 KB / 42, voice 44 KB / 42 (compressed). The
  three tarballs install into a scratch app with no warning, `@sinua/core` deduplicated
  under the exact peer. `resolveFxSpec` resolves and a frame renders (516 dots); `web` and
  `voice` import. `--publishable` refuses the `<REPO_URL>` placeholder, as it should.
- **Maven:** seven modules staged at `0.1.0-beta.1`. Signed with a throwaway key: 35
  artefacts, 35 `.asc`, `gpg --verify` good. A scratch Android library resolving the
  staging directory compiles `SinuaOrb` and a UniFFI `resolveFxSpec` call. That check
  found a real bug: `sinua-view` had compose-ui only on the *runtime* classpath, so a
  consumer couldn't compile `Modifier`. It is `api` now (`view/build.gradle.kts`).
- **SwiftPM:** the zip is 23 MB. Its checksum (`9ad7e03c…af45`) is its sha256 and comes
  out the same on a rebuild. All three generated manifests parse, and a scratch app
  builds `Sinua` + `SinuaVoice` for the simulator from `swift-local/sinua-swift`.
- **release.yml:** `actionlint` is clean (with the custom `xcode-27` runner label
  allowed). It hasn't run on GitHub yet: the Actions quota is spent until 1 October.

## npm packages

Verified 2026-09-20 by packing each one, installing the tarballs into a scratch app, and
importing them under **both** `moduleResolution: bundler` and `node16` with
`skipLibCheck: false`, then running the result in node.

Sizes are `npm pack --dry-run` unpacked size, re-measured 2026-09-20.

| Package | Tarball | Ships | Notes |
|---|---|---|---|
| `@sinua/core` | 473 KB, 22 files | `dist/`, `pkg/*.js` + `*.d.ts`, licences, README | the wasm is inlined in `pkg/sinua_core_inline.js`; `sideEffects` lists exactly that file, because marking the package side-effect-free lets bundlers drop its `initSync` (see `bundler-check/`) |
| `@sinua/web` | 153 KB, 42 files | `dist/` (including the generated typed components and elements), framework typings, README | `sideEffects` lists `./dist/element-define.js` — a package-wide `false` let bundlers drop `import "@sinua/web/element"` whole, so `<sinua-view>` was never registered and nothing errored (found by studio-ui-ux, 2026-09-20; `bundler-check` now covers it). The `./components` and `./elements` subpaths are generated |
| `@sinua/voice` | 153 KB, 42 files | `dist/` per subpath, README | `livekit-client` is an optional peer |
| `@sinua/snippets` | 30 KB, 6 files | `dist/`, README | |
| `@sinua/react-native` | **77 MB**, 134 files | `dist/`, `src/`, `ios/`, `android/src`, `android/build.gradle.kts`, the xcframework, the podspec, README | **was 52 MB / 3592 files**: `files` had the whole `android/` directory, which pulled Gradle's caches and 78 MB of build output. **Open question for release:** 69 MB of the 77 is the two iOS static archives (`core_engineFFI.xcframework`, 46 MB simulator + 23 MB device). They are `--release` builds, not debug -- a static archive simply carries every object, and the app's linker drops what it doesn't use -- so this is not a build mistake. It is still a 77 MB install for every consumer, and the simulator slice in particular is a candidate for shipping separately. Not decided |

Filled: `author` on all five, matching `NOTICE`'s authors string exactly (the two
drifted apart once), `keywords`, and `repository` (with its `directory`), `homepage` and
`bugs` on the public repository (2026-09-22). `private: true` stays in the tree on purpose: `npm-pack.mjs` removes it only in the
packed copy, and `packages/core/test/manifests.test.mjs` still fails the build if
`private` comes off a manifest that has a placeholder. `@sinua/snippets` and
`@sinua/react-native` don't publish (decision 0.5); the pipeline packs only core, web and
voice.

Present and checked: `files`, `exports` (every subpath resolves under both resolutions),
`types`, `engines: node >= 20`, `sideEffects`, LICENSE + NOTICE, and a README each.
Source maps are not emitted; the `dist` is plain readable ESM, so that is a deliberate gap
rather than an oversight — revisit if consumers ask.

## Android (Maven)

`gradle/publish.gradle.kts` (applied by the seven library modules) adds `maven-publish`
with the POM Maven Central requires: name, description, url, licence, developers and SCM.
Each module declares its published variant with `withSourcesJar()` + `withJavadocJar()`.
The version is `sinua.version` from `gradle.properties` (set by `set-version.mjs`;
missing is an error, not a default). Two properties drive a release:

- `-Psinua.stagingRepo=<dir>` adds a `staging` repository, so
  `publishAllPublicationsToStagingRepository` writes a Central-shaped directory;
- `ORG_GRADLE_PROJECT_signingKey` (+ `signingPassword`) turns on the `signing`
  plugin with the key held in memory. Without it the build says it is **UNSIGNED**, fine
  for a dry run and refused by Central (and by `build.sh --publishable`).

Verified: see the dry run above. `publishToMavenLocal` still works for local testing.

Still missing: the Central Portal account and the verified `dev.sinua` namespace, and
the real signing key. The vendor modules publish with the rest, marked beta (decision 0.3).

## iOS (Swift Package Manager)

Remote consumers get the `sinua-dev/sinua-swift` repository (above, *Why three SwiftPM
repositories*). Inside this repository nothing changes: `packages/ios/Package.swift` keeps the
local `path:` binary target, and the apps and tests build against it by path.

CocoaPods (used by React Native) is separate: `SinuaCore.podspec` carries the pod's
version, source and subspecs, and its `source` URL is brand-dependent.

## Rust (crates.io): deliberately nothing

The engine reaches users as `@sinua/core` (wasm), the SwiftPM package and the Maven
artifacts — never as a crate. Both workspace crates therefore set `publish = false`
(`crates/core_engine/Cargo.toml`, `crates/uniffi-bindgen/Cargo.toml`), so `cargo publish`
from the workspace root cannot release them. `core_engine` was publishable by default
until 2026-09-20 (`docs/code-health-engine.md` E7).

`crates/core_engine/tests/manifests.rs` holds this: every crate under `crates/` must
declare `publish = false`, no manifest may carry the old brand, and the workspace
`authors` string must appear in `NOTICE` — the three drifted apart once already.
Publishing a crate on purpose means editing that test as well as the manifest, which is
the intent: it should be a visible decision, not a default.

## Decided: FX Spec 1.8 is the floor (done 2026-09-21)

The one decision here that was cheap only until the first publish: whether the
resolver's acceptance of FX Spec 1.0–1.7 ships. Once published, it would have become a
compatibility promise, and removing it afterwards a major version. The user decided to
drop it (`docs/release-roadmap.md`, decision 0.1), and it is done:

- **The runtime reads 1.8 and later** (`FLOOR_MINOR`). A 1.0–1.7 file is one error at
  `/fxSpec`. The names 1.7 replaced (`state`, engine-key binding targets) are errors that
  name the replacement. The taxonomy retrofit's per-mode aliases (`legacy_rename`, 43
  pairs) are gone; `web`'s current `nodeN` is unaffected.
- **The resolver shrank** from 1,744 to 1,541 lines outside its tests, with all 12 version
  gates removed. The 1.0–1.7 identity locks went too; `spec/fx-spec-1.8-resolved.json`
  is the one lock left.
- **The examples went from 16 to 13, all 1.8.** `migration-legacy-names` and the two
  `-1.7` twins went, and the rest were converted by the Studio's own converter. Each one
  was checked to resolve like its original in every state and power state, except where
  1.8's voice-state profile now fills a voice-state entry.
- **The Studios moved first.** They keep their engine-name document but hand the runtime
  only 1.8 files (`toSpecFile`, all three platforms), so they drew and exported through the
  whole change.

## The order on release day

0. ~~Answer the FX Spec floor question.~~ Done: 1.8 is the floor (above).
1. The accounts: sinua.dev, the npm org, the `sinua-dev` GitHub org with this repository
   and its three SwiftPM repositories, the verified `dev.sinua` namespace; the secrets in
   *How to release*.
2. A manual run of **Release**: the dry run on GitHub's runners.
3. `git tag v<VERSION>` and push. The three jobs publish.
4. Re-run the dry run's checks against the **published** artefacts, not the local ones.
