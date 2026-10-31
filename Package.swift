// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "Scanner",
    platforms: [.macOS(.v14)],
    products: [.library(name: "ScannerCore", targets: ["ScannerCore"])],
    targets: [
        .target(name: "ScannerAPI"),
        .target(name: "ScannerCore", dependencies: ["ScannerAPI"]),
        .target(name: "ScannerTransport", dependencies: ["ScannerAPI"]),
        .target(name: "ScannerServiceSupport", dependencies: ["ScannerCore", "ScannerAPI"]),
        .testTarget(name: "ScannerAPITests", dependencies: ["ScannerServiceSupport", "ScannerTransport", "ScannerAPI"]),
        .testTarget(name: "ScannerCoreTests", dependencies: ["ScannerCore", "ScannerAPI"])
    ]
)
