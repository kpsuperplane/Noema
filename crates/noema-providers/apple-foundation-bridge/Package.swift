// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "NoemaFoundationBridge",
    platforms: [
        .macOS("26.0")
    ],
    products: [
        .executable(name: "noema-foundation-bridge", targets: ["NoemaFoundationBridge"])
    ],
    targets: [
        .executableTarget(name: "NoemaFoundationBridge")
    ]
)
