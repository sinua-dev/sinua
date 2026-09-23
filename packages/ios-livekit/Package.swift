// swift-tools-version:5.9
import PackageDescription

// The native LiveKit `VoiceSource` (docs/audio-pipeline.md, *Native LiveKit*).
// Its own package, not a product of packages/ios: SwiftPM resolves every
// declared dependency of a package, so putting LiveKit there would make every
// SinuaView consumer fetch the SDK and its WebRTC binary even if they never link
// this. The SDK-free logic (`LiveKitAgentTracker`) lives in SinuaVoice and
// is tested there; this package is only the Room/track glue.
let package = Package(
    name: "SinuaLiveKit",
    platforms: [.iOS(.v15)],
    products: [
        .library(name: "SinuaLiveKit", targets: ["SinuaLiveKit"])
    ],
    dependencies: [
        .package(path: "../ios"),
        .package(url: "https://github.com/livekit/client-sdk-swift.git", from: "2.17.0"),
    ],
    targets: [
        .target(
            name: "SinuaLiveKit",
            dependencies: [
                .product(name: "SinuaVoice", package: "ios"),
                .product(name: "LiveKit", package: "client-sdk-swift"),
            ],
            path: "Sources/SinuaLiveKit"
        )
    ]
)
