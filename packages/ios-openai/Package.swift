// swift-tools-version:5.9
import PackageDescription

// The native OpenAI Realtime VoiceSource over WebRTC (docs/audio-pipeline.md,
// *Native OpenAI Realtime*). Its own package, like packages/ios-livekit: it
// carries a WebRTC binary that SinuaView and the other vendors must never pull.
// WebRTC is LiveKit's prefixed build -- `from:` the version client-sdk-swift
// 2.17.0 pins exactly, so an app with both resolves to one binary.
let package = Package(
    name: "SinuaOpenAI",
    platforms: [.iOS(.v15)],
    products: [
        .library(name: "SinuaOpenAI", targets: ["SinuaOpenAI"])
    ],
    dependencies: [
        .package(path: "../ios"),
        .package(url: "https://github.com/livekit/webrtc-xcframework.git", from: "150.7871.02"),
    ],
    targets: [
        .target(
            name: "SinuaOpenAI",
            dependencies: [
                .product(name: "SinuaVoice", package: "ios"),
                .product(name: "LiveKitWebRTC", package: "webrtc-xcframework"),
            ],
            path: "Sources/SinuaOpenAI"
        )
    ]
)
