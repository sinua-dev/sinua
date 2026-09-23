// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "Sinua",
    platforms: [.iOS(.v15)],
    products: [
        .library(name: "CoreEngine", targets: ["CoreEngine"]),
        // Native audio -> the engine's voice opts (docs/audio-pipeline.md, *Native*).
        // Separate product so an app that never touches the mic doesn't link it.
        .library(name: "SinuaVoice", targets: ["SinuaVoice"]),
        // Drop-in SwiftUI renderer: SinuaView(spec:voice:) + the shared paint contract (docs/fx-view.md).
        .library(name: "Sinua", targets: ["Sinua"]),
        // Gemini Live VoiceSource (docs/audio-pipeline.md, *Native Gemini Live*).
        // Foundation-only (URLSessionWebSocketTask); its own product so the
        // socket code isn't linked by apps that don't use it.
        .library(name: "SinuaGeminiLive", targets: ["SinuaGeminiLive"]),
        // ElevenLabs Agents VoiceSource (docs/audio-pipeline.md, *Native ElevenLabs*); Foundation only.
        .library(name: "SinuaElevenLabs", targets: ["SinuaElevenLabs"]),
    ],
    targets: [
        .binaryTarget(name: "core_engineFFI", path: "core_engineFFI.xcframework"),
        .target(
            name: "CoreEngine",
            dependencies: ["core_engineFFI"],
            path: "Sources/CoreEngine"
        ),
        // The voice *types* only (VoiceSource, VoiceOverrides, AgentState): Foundation, no
        // audio I/O. The views depend on this, not on SinuaVoice, so an app that only draws
        // doesn't link the microphone code (release decision 0.4). Internal:
        // SinuaVoice and Sinua both re-export it, so it's never imported directly.
        .target(
            name: "SinuaVoiceTypes",
            path: "Sources/SinuaVoiceTypes"
        ),
        .target(
            name: "SinuaVoice",
            dependencies: ["SinuaVoiceTypes"],
            path: "Sources/SinuaVoice"
        ),
        .target(
            name: "Sinua",
            dependencies: ["CoreEngine", "SinuaVoiceTypes"],
            path: "Sources/Sinua"
        ),
        .target(
            name: "SinuaGeminiLive",
            dependencies: ["SinuaVoice"],
            path: "Sources/SinuaGeminiLive"
        ),
        .target(
            name: "SinuaElevenLabs",
            dependencies: ["SinuaVoice"],
            path: "Sources/SinuaElevenLabs"
        ),
        .testTarget(
            name: "CoreEngineTests",
            dependencies: ["CoreEngine"],
            path: "Tests/CoreEngineTests"
        ),
        .testTarget(
            name: "SinuaVoiceTests",
            dependencies: ["SinuaVoice"],
            path: "Tests/SinuaVoiceTests"
        ),
        .testTarget(
            name: "SinuaTests",
            dependencies: ["Sinua", "CoreEngine", "SinuaVoice"],
            path: "Tests/SinuaTests"
        ),
        .testTarget(
            name: "SinuaGeminiLiveTests",
            dependencies: ["SinuaGeminiLive", "SinuaVoice"],
            path: "Tests/SinuaGeminiLiveTests"
        ),
        .testTarget(
            name: "SinuaElevenLabsTests",
            dependencies: ["SinuaElevenLabs", "SinuaVoice"],
            path: "Tests/SinuaElevenLabsTests"
        ),
    ]
)
