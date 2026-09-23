#!/usr/bin/env bash
# The Swift gate for packages/ios, packages/ios-livekit and packages/ios-openai.
#
#   scripts/lint-swift.sh            # check; non-zero if anything is unformatted
#   scripts/lint-swift.sh --fix      # format in place
#   scripts/lint-swift.sh <dir>...   # also lint these (an app built on the runtime;
#                                    # swift-format reads the nearest .swift-format)
#
# It uses `swift format`, which **ships with the Swift toolchain** (6.x) -- so this
# gate costs no new dependency: nothing to install locally, nothing to install in
# CI. SwiftLint would have been a separate binary for a similar result.
#
# ---------------------------------------------------------------- the config --
# `.swift-format` at the repo root; swift-format searches upward, so one file
# covers all three packages. It is the full default rule set (31 of 43 rules
# enabled, swift-format's own defaults) with exactly **two** deviations, and
# since its config format is strict JSON and cannot carry comments, the reasons
# live here:
#
#   indentation: 4 spaces (default 2)  -- what this codebase has always used.
#       Left at the default it produced 9 137 of 10 750 findings, i.e. the gate
#       would have been a demand to reindent everything rather than a check.
#   lineLength: 120 (default 100)      -- measured: p50 36, p95 101, p99 132.
#       100 would have reflowed a long tail of deliberately-shaped call sites for
#       no benefit; 120 covers ~99% of the code as written.
#
# Nothing is suppressed beyond those two. If a rule ever needs turning off, put
# the count and the reason next to it here -- a suppression with no reason
# beside it is the thing the next person deletes or cargo-cults.
#
# ------------------------------------------------------------- the exclusions --
# Generated Swift is excluded, and this is the part that matters: reformatting it
# would be undone by the next generator run, and would make the generator's own
# freshness check fail.
#
#   Sources/Sinua/Generated/   -- scripts/codegen/generate.mjs (`--check` in CI)
#   Sources/CoreEngine/       -- UniFFI, written by packages/ios/build.sh (the whole
#                                directory, by path: excluding the *filename*
#                                core_engine.swift also skipped any hand-written file
#                                that happened to share the name, and would have linted
#                                a second generated file if build.sh ever emitted one.
#                                Both proved on a scratch copy, 2026-09-20.)
#
# swift-format has no ignore-file mechanism, so the exclusion is here and only
# here. An app's build/ can hold hundreds of vendored LiveKit Swift files under
# build/dd/SourcePackages, which `-not -path '*/build/*'` already drops -- the
# success line's file count is how to tell.
set -euo pipefail

# Extra directories after the flags are linted with the same config, for a tree
# that builds on this one (an app in another repository). Made absolute before
# the cd below.
MODE="lint"
EXTRA=()
for arg in "$@"; do
  case "$arg" in
    --fix) MODE="format" ;;
    -*) echo "usage: $0 [--fix] [extra dir...]" >&2; exit 2 ;;
    *) EXTRA+=("$(cd "$arg" && pwd)") ;;
  esac
done
cd "$(dirname "$0")/.."

# `mapfile` is bash 4; macOS ships 3.2, where it silently does nothing and leaves
# the array empty -- so collect with a read loop instead. (Caught by running this,
# not by reading it.)
FILES=()
while IFS= read -r f; do
  FILES+=("$f")
done < <(
  find packages/ios packages/ios-livekit packages/ios-openai ${EXTRA[@]+"${EXTRA[@]}"} \
    -name '*.swift' \
    -not -path '*/.build/*' \
    -not -path '*/build/*' \
    -not -path '*/Generated/*' \
    -not -path '*/Sources/CoreEngine/*' \
    | sort
)

if [ "${#FILES[@]}" -eq 0 ]; then
  echo "lint-swift: no Swift sources found -- wrong directory?" >&2
  exit 1
fi

if [ "$MODE" = "format" ]; then
  swift format --in-place --parallel "${FILES[@]}"
  echo "lint-swift: formatted ${#FILES[@]} files"
else
  # --strict turns findings into errors, which is what makes this a gate rather
  # than a report.
  swift format lint --strict --parallel "${FILES[@]}"
  echo "lint-swift: ${#FILES[@]} files clean"
fi
