// swift-tools-version: 6.0
import PackageDescription

let package = Package(
    name: "Scanner",
    platforms: [.macOS(.v14)],
    products: [
        .library(name: "ScannerCore", targets: ["ScannerCore"]),
        .executable(name: "scanner", targets: ["ScannerCLI"]),
        .executable(name: "scanner-service", targets: ["ScannerService"])
    ],
    dependencies: [
        .package(url: "https://github.com/apple/swift-argument-parser", from: "1.7.0"),
        .package(url: "https://github.com/groue/GRDB.swift.git", from: "7.11.1")
    ],
    targets: [
        .executableTarget(name: "ScannerCLI", dependencies: ["ScannerAPI", "ScannerTransport", .product(name: "ArgumentParser", package: "swift-argument-parser")]),
        .executableTarget(name: "ScannerService", dependencies: ["ScannerAPI", "ScannerCore", "ScannerPersistence", "ScannerTransport", "ScannerServiceSupport", .product(name: "ArgumentParser", package: "swift-argument-parser")]),
        .target(name: "ScannerAPI"),
        .target(name: "ScannerCore", dependencies: ["ScannerAPI", "ScannerPersistence"]),
        .target(name: "ScannerPersistence", dependencies: ["ScannerAPI", .product(name: "GRDB", package: "GRDB.swift")]),
        .target(name: "ScannerTransport", dependencies: ["ScannerAPI"]),
        .target(name: "ScannerServiceSupport", dependencies: ["ScannerCore", "ScannerAPI"]),
        .testTarget(name: "ScannerAPITests", dependencies: ["ScannerCore", "ScannerServiceSupport", "ScannerTransport", "ScannerAPI"]),
        .testTarget(name: "ScannerCLITests", dependencies: ["ScannerCLI", "ScannerAPI"]),
        .testTarget(name: "ScannerCoreTests", dependencies: ["ScannerCore", "ScannerAPI", "ScannerPersistence"]),
        .testTarget(name: "ScannerPersistenceTests", dependencies: ["ScannerPersistence", "ScannerAPI"])
    ]
)
