# External oracle for the orbs family

`spec/orbs-golden.json` is upstream `thinking-orbs`' frozen golden, vendored verbatim. It
covers sizes 64 and 20 only: it was captured at upstream `9d735d1` (2026-08-16), before
upstream had size 32 (added in `91416d9`, 2026-08-31). `spec/orbs-golden-32.json` extends
the same oracle, upstream's own engine run unmodified, to size 32.
`crates/core_engine/tests/golden.rs` holds our port to both files.

## Reproduce `spec/orbs-golden-32.json`

```sh
git clone https://github.com/Jakubantalik/Libraries.dev /tmp/libraries-dev
git -C /tmp/libraries-dev checkout 2015f0ba79a9faec351719c4a6d590a1e6bfa243
UP=/tmp/libraries-dev/packages/thinking-orbs

# 1. The commit is a valid oracle only if it reproduces the vendored file exactly.
npx tsx scripts/oracle/extract-orbs-golden.ts "$UP" --sizes 64,20 | cmp - spec/orbs-golden.json

# 2. Then size 32 from the same commit and the same loop.
npx tsx scripts/oracle/extract-orbs-golden.ts "$UP" --sizes 32 | cmp - spec/orbs-golden-32.json
```

No `npm install` is needed. The engine files import nothing at runtime (`react` appears
only as types, which `tsx` strips).

## Why a later commit is still the same oracle

Measured 2026-09-21 at `2015f0b`:

- Step 1 passes byte for byte (sha256 `70bfaa2b…`).
- Upstream's `presets.ts` hasn't changed since `91416d9`, the commit our 32 presets
  were ported from.
- The engine changed in two opt-in ways that no preset reaches: morph's `shape` knob
  (unset leaves it as before) and `countDots` (not called while drawing).

Moving to a newer upstream commit means re-running both steps. If step 1 fails, upstream's
engine moved. Which version counts as the oracle is then a decision for the user, not
something to pick quietly (families LOG, 2026-09-21 11:23).
