// Package.swift -- from the first beta. Until it is published, use install/from-source.sh.
dependencies: [
    .package(url: "https://github.com/sinua-dev/sinua-swift", from: "0.1.0-beta.1"),
],
targets: [
    .target(name: "MyApp", dependencies: [
        .product(name: "Sinua", package: "sinua-swift"),
        .product(name: "SinuaVoice", package: "sinua-swift"),
    ]),
]
