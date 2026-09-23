# Sinua

One visual-effects engine, written once in Rust and drawn identically on Web,
iOS, Android and React Native. It renders animated visuals for voice agents and
assistants — a listening orb, an audio-reactive strip, a progress ring — and the
four platforms are proven identical by testing them against the same frozen
vectors, not by reading the code side by side.

**5 objects, 34 patterns:** orb (18), signal (4), ring (5), core (2), beacon (5).
A design is either a few typed props or an **FX Spec** file (`.fxspec.json`,
currently 1.8) that every platform resolves the same way.

> **Nothing is published yet.** The first public beta (`0.1.0-beta.1`) goes to npm,
> Maven Central and SwiftPM through this repository's release pipeline; until then,
> build from source. See [docs/publishing.md](docs/publishing.md).

## Licensing

- **Apache-2.0**: the engine (`crates/`), every platform package (`packages/`) and
  everything else in this repository. That is the [`LICENSE`](LICENSE) at the root,
  and every published package declares it.
- **Attribution:** part of the engine is ported from
  [thinking-orbs](https://github.com/Jakubantalik/Libraries.dev) by Jakub
  Antalik, under the MIT licence. See [`NOTICE`](NOTICE) and
  [`third_party/thinking-orbs/LICENSE`](third_party/thinking-orbs/LICENSE).
- **The Studio**, the hosted design tool that exports FX Spec files and code, is a
  separate commercial product and is not in this repository. Nothing here needs it:
  every design can be written by hand as props or an FX Spec file.

## Build from source

Everything needs Rust; the toolchain is pinned by `rust-toolchain.toml`, so
`rustup` picks the right one inside this repo. For the Web path you also need
`wasm-pack` and Node.

```bash
cargo test -p core_engine        # the engine and its golden vectors
scripts/ci-local.sh --list       # what a full local check covers, without running it
scripts/ci-local.sh              # the rust + web jobs
scripts/ci-local.sh --all        # every job CI runs, including the native legs
```

[docs/development.md](docs/development.md) has the per-platform toolchains — iOS,
Android and React Native each need their own — in the order they have to be
installed.

## Where things are

| | |
|---|---|
| `crates/core_engine` | the engine: geometry, patterns, materials, the FX Spec resolver |
| `packages/` | what consumers install: `@sinua/{core,web,voice,snippets,react-native}` on npm, the SwiftPM package `Sinua`, `dev.sinua:sinua-*` on Maven, the `SinuaCore` pod |
| `spec/` | the shared data every platform is tested against: golden vectors, FX Spec identity locks, the parameter catalog |
| `apps/site` | the documentation site |
| `apps/examples` | `<sinua-view>` in plain HTML, Vue, Svelte, Solid and Angular |
| `scripts/` | codegen, docs checks, and the CI scripts both GitHub Actions and `ci-local.sh` run |
| `docs/` | the engineering documentation — start at [docs/README.md](docs/README.md) |

## Reading next

- [docs/README.md](docs/README.md) — the index, and the fastest way in.
- [docs/architecture.md](docs/architecture.md) — why Rust, the family pattern,
  and the UniFFI/wasm-bindgen split every binding is built on.
- [docs/fx-spec.md](docs/fx-spec.md) — the file format, its versioning, and the
  identity locks that hold older files to their original meaning.
- [docs/testing.md](docs/testing.md) — the golden sets, what they do and do not
  prove, and what CI actually runs.
