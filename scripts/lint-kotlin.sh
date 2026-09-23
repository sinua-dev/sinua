#!/usr/bin/env bash
# The Kotlin gate for packages/android.
#
#   scripts/lint-kotlin.sh           # check; non-zero if anything is unformatted
#   scripts/lint-kotlin.sh --fix     # format in place
#   scripts/lint-kotlin.sh <dir>...  # also lint these (an app built on the runtime;
#                                    # ktlint reads the nearest .editorconfig)
#
# Sibling of scripts/lint-swift.sh, same shape. Style lives in the repo root's
# `.editorconfig` under [*.kt], with the measurements that chose it.
#
# -------------------------------------------------------------------- ktlint --
# ktlint is the formatter/style linter -- the direct analogue of `swift format`.
# detekt is a different job (static analysis for code smells) and is worth
# proposing on its own rather than bundling in here.
#
# The version is **pinned** and the binary is fetched into a gitignored cache on
# first use, rather than adding a Gradle plugin. Three areas build
# packages/android; a plugin there would change the build for all of them, while
# a pinned CLI touches nothing but this script. The cost is one download per
# machine.
KTLINT_VERSION="1.8.0"

# ---------------------------------------------------------------- exclusions --
# Generated Kotlin is never linted or formatted:
#
#   **/uniffi/      -- UniFFI bindings, written by packages/android/build.sh
#   **/generated/   -- scripts/codegen/generate.mjs (`--check` fails CI if stale)
#
# Reformatting either is undone by the next generator run, and would make the
# generator's own freshness check fail. apps/fx-bench-android is still out of
# scope. The success line's file count is how to tell a directory was taken.
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

CACHE=".cache/ktlint"
KTLINT="$CACHE/ktlint-$KTLINT_VERSION"
if [ ! -x "$KTLINT" ]; then
  echo "lint-kotlin: fetching ktlint $KTLINT_VERSION"
  mkdir -p "$CACHE"
  # Retried: GitHub's release downloads return transient 504s, and one of them failed a
  # whole CI run on 2026-09-21 before anything was linted.
  curl -sSL --fail --retry 5 --retry-delay 5 --retry-all-errors -o "$KTLINT" \
    "https://github.com/pinterest/ktlint/releases/download/$KTLINT_VERSION/ktlint"
  chmod +x "$KTLINT"
fi

# `mapfile` is bash 4 and macOS ships 3.2, where it is not a command at all and
# leaves the array empty -- a gate that passes because it linted nothing. Same
# reason as lint-swift.sh; please don't "simplify" this back.
FILES=()
while IFS= read -r f; do
  FILES+=("$f")
done < <(
  find packages/android ${EXTRA[@]+"${EXTRA[@]}"} \
    -name '*.kt' \
    -not -path '*/build/*' \
    -not -path '*/uniffi/*' \
    -not -path '*/generated/*' \
    | sort
)

if [ "${#FILES[@]}" -eq 0 ]; then
  echo "lint-kotlin: no Kotlin sources found -- wrong directory?" >&2
  exit 1
fi

if [ "$MODE" = "format" ]; then
  "$KTLINT" --format "${FILES[@]}" || true
  echo "lint-kotlin: formatted ${#FILES[@]} files"
else
  "$KTLINT" --relative "${FILES[@]}"
  echo "lint-kotlin: ${#FILES[@]} files clean"
fi
