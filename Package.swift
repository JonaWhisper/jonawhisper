// swift-tools-version: 5.9

import PackageDescription

let package = Package(
    name: "WhisperDictate",
    platforms: [.macOS(.v13)],
    targets: [
        .executableTarget(
            name: "WhisperDictate",
            path: "Sources/WhisperDictate",
            linkerSettings: [
                .linkedFramework("Cocoa"),
                .linkedFramework("AVFoundation"),
                .linkedFramework("Accelerate"),
                .linkedFramework("CoreAudio"),
                .linkedFramework("CoreGraphics"),
                .linkedFramework("UserNotifications"),
            ]
        )
    ]
)
