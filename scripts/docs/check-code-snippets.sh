#!/usr/bin/env bash
# Compiles the docs site's code samples (apps/site/snippets/) so they can't drift from the API.
#   --web     TypeScript/TSX (Web + React Native) type-check, install sample names  (needs packages/{core,web,voice} built)
#   --native  Swift (xcodebuild, iOS Simulator) + Kotlin/XML (:site-snippets)       (needs the xcframework / .so built)
#   --native-swift  Swift only (scripts/ci-native-ios.sh)
# spec/ and ts/ samples are checked by scripts/docs/check-snippets.mjs.
set -euo pipefail
cd "$(dirname "$0")/../.."
TSC=packages/core/node_modules/.bin/tsc
case "${1:-}" in
  --web)
    "$TSC" -p apps/site/snippets/tsconfig.web.json && echo "web snippets: type-check ok"
    "$TSC" -p apps/site/snippets/tsconfig.rn.json && echo "react-native snippets: type-check ok"
    node scripts/docs/check-install-snippets.mjs
    ;;
  --native|--native-swift)
    (cd apps/site/snippets && xcodebuild build -quiet -scheme SiteSnippets -destination 'generic/platform=iOS Simulator' \
      -derivedDataPath "${SNIPPETS_DERIVED_DATA:-build/snippets-derived-data}") && echo "swift snippets: build ok"
    [ "$1" = --native-swift ] && exit 0   # ci-native-android.sh builds :site-snippets with its other modules
    (cd packages/android && ./gradlew --no-daemon -q :site-snippets:assembleDebug) && echo "kotlin/xml snippets: build ok"
    ;;
  *) echo "usage: $0 --web | --native" >&2; exit 2 ;;
esac
