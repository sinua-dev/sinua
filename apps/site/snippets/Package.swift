// swift-tools-version:5.9
// Compiles the docs' Swift samples (apps/site/snippets/**/*.swift) against the local
// packages, so a sample that drifts from the API fails CI (scripts/docs/check-code-snippets.sh
// --native). Nothing links it; the library product only gives xcodebuild a scheme.
import PackageDescription

/// Each folder's non-Swift samples (TS, Kotlin, XML, ...) are excluded from its target.
let topics: [(folder: String, target: String, exclude: [String])] = [
    ("first-visual", "FirstVisual", ["react.tsx", "react-native.tsx", "compose.kt"]),
    ("platforms", "Platforms", ["compose.kt", "android-view.kt", "android-res", "react-native.tsx"]),
    ("values", "Values", ["react.tsx", "compose.kt"]),
    ("states", "States", ["react.tsx", "compose.kt", "plain-react.tsx", "plain-compose.kt", "plain-react-native.tsx"]),
    ("bindings", "Bindings", ["react.tsx", "compose.kt"]),
    ("voice", "Voice", ["overview-web.ts", "openai-realtime-web.ts", "gemini-live-web.ts", "elevenlabs-web.ts", "livekit-web.ts", "mic-and-tone-web.ts",
                        "overview.kt", "openai-realtime.kt", "gemini-live.kt", "elevenlabs.kt", "livekit.kt", "mic-and-tone.kt"]),
    ("perf", "Perf", ["low-power-web.ts", "max-fps-web.ts", "reduced-motion-web.ts", "accessibility-label-web.ts",
                      "low-power.kt", "max-fps.kt", "reduced-motion.kt", "accessibility-label.kt"]),
]

let package = Package(
    name: "SiteSnippets",
    platforms: [.iOS(.v15)],
    products: [.library(name: "SiteSnippets", targets: topics.map(\.target))],
    dependencies: [
        .package(path: "../../../packages/ios"),
        .package(path: "../../../packages/ios-livekit"),
        .package(path: "../../../packages/ios-openai"),
        .package(url: "https://github.com/livekit/client-sdk-swift.git", from: "2.17.0"),
    ],
    // One target per topic folder: the file names repeat across folders (swiftui.swift, ...).
    targets: topics.map { topic in
        .target(
            name: topic.target,
            dependencies: [
                .product(name: "Sinua", package: "ios"),
                .product(name: "SinuaVoice", package: "ios"),
                .product(name: "SinuaGeminiLive", package: "ios"),
                .product(name: "SinuaElevenLabs", package: "ios"),
                .product(name: "SinuaLiveKit", package: "ios-livekit"),
                .product(name: "SinuaOpenAI", package: "ios-openai"),
                .product(name: "LiveKit", package: "client-sdk-swift"),
            ],
            path: topic.folder,
            exclude: topic.exclude
        )
    }
)
