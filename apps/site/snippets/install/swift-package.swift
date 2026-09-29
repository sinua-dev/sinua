// Package.swift -- the SwiftPM package is github.com/sinua-dev/sinua-swift.
dependencies: [
    .package(url: "https://github.com/sinua-dev/sinua-swift", from: "0.1.0-beta.6"),
],
targets: [
    .target(name: "MyApp", dependencies: [
        .product(name: "Sinua", package: "sinua-swift"),
        .product(name: "SinuaVoice", package: "sinua-swift"),
    ]),
]
