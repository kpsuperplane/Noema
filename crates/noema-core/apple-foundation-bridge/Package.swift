// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "NoemaFoundationBridge",
    platforms: [
        .macOS(.v15)
    ],
    products: [
        .executable(name: "noema-foundation-bridge", targets: ["NoemaFoundationBridge"])
    ],
    targets: [
        .executableTarget(name: "NoemaFoundationBridge")
    ]
)
