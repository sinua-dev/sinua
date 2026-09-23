# Materials cross-platform check

Does the Web, iOS and Android painter draw the **same frame**? This is the
pixel half of that claim — geometry, fills, gradients, blur, additive blend,
per-vertex stroke colour. The colour half is `spec/paint-ink-vectors.json`,
checked inside the native test suites.

```
scripts/materials-check.sh                # both natives, then compare
scripts/materials-check.sh --render-ios   # also drive MaterialsRenderTests first
scripts/materials-check.sh --ios-only     # or --android-only
scripts/materials-check.sh --compare-only # CI: everything is already staged
```

It exits 0 only when every pair is within tolerance **and** every render it was
told to expect actually arrived. Needs Playwright + a Chromium for the Web
render (`PLAYWRIGHT` / `CHROME`, the same variables `gen-ink-vectors.mjs`
takes). It never boots an emulator and never rebuilds the xcframework.

## What runs where

| | produced by | collected by |
|---|---|---|
| Web frames | `frames.mjs` (the golden file's materials cases + `SYNTHETIC`) | — |
| iOS renders | `MaterialsRenderTests` (already inside `-only-testing:SinuaTests`) | `collect-ios.mjs` from the `.xcresult` |
| Android renders | `renderMaterialsGoldenCases` (already inside `:sinua-view:connectedDebugAndroidTest`) | `collect-android.mjs` from Gradle's per-test logcat |
| Comparison | `compare.cjs` in Chrome | — |

Both native render tests **already run on every CI push**; until 2026-09-20
their output was produced and thrown away. The `materials` job in
`.github/workflows/ci.yml` collects it, so the check costs one Chromium and no
device time.

`collect-android.mjs` reads
`packages/android/view/build/outputs/androidTest-results/…/logcat-…renderMaterialsGoldenCases.txt`,
which Gradle writes itself — not `adb logcat`. The renders leave the device as
base64 chunks (`name|i|chunk`, then `name|END|<len>`) because the test APK's own
files are deleted with it, and the live buffer is a shared, wrapping ring: 1.3 MB
of chunks against a 256 KB default. Every `|END|<len>` is verified, and a short
or missing chunk fails **by name**.

## Tolerance

`docs/fx-view.md`, *Fills and effects*: composite over the theme paper, compare
per RGB channel, **mean ≤ 2/255 and p99 ≤ 24/255**. These numbers are a decision,
not a default; `packages/web/test/materials-cases.test.mjs` pins them so changing
one has to be deliberate.

**Anti-aliasing exceptions.** `AA_EXCEPTIONS` in `compare.cjs` lists the cases
whose full-resolution p99 is dominated by anti-aliasing differences between
Chrome and CoreGraphics/Skia. Today that is only
`drifting-64-0.6-particles-liquid`. A listed case passes on full-resolution
mean ≤ 2 **and** half-resolution (2×2 box) p99 ≤ 24; its full-resolution p99 is
still reported, as information. Adding a case is a deliberate decision that
needs a reason, the same as a golden re-baseline; the list is agreed with
families, and the same test pins it.

## A missing render is a failure

Until 2026-09-20 it was not. A platform with no PNG was recorded as the *string*
`"missing"`, and the verdict loop only inspected object values — so an **empty
directory printed 42 rows of `"missing"` followed by `ALL WITHIN TOLERANCE` and
exit 0**. `compare.cjs` now takes `--platforms` and `--expect`: the run declares
what it is a statement about, and anything expected but absent fails with a
name. A cross-platform check that passes when neither platform rendered is worse
than no check.

The case list is written three times — derived here from
`spec/sinua-golden.json`, and hard-coded in `MaterialsRenderTests.swift` and
`SinuaViewTest.kt`. `packages/web/test/materials-cases.test.mjs` compares the
three as **sets**, so a case added on one side only fails too, not just a case
dropped. Keep them in sync when new materials cases land; the test will say
which name is where.
