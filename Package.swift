// swift-tools-version: 6.2
import PackageDescription

let package = Package(
    name: "Scanner",
    platforms: [.macOS(.v14)],
    products: [
        .library(name: "ScannerCore", targets: ["ScannerCore"]),
        .executable(name: "scanner", targets: ["ScannerCLI"]),
        .executable(name: "scanner-service", targets: ["ScannerService"]),
        .executable(name: "scanner-mcp", targets: ["ScannerMCP"]),
        .executable(name: "scanner-http", targets: ["ScannerHTTP"])
    ],
    dependencies: [
        .package(url: "https://github.com/apple/swift-argument-parser", from: "1.7.0"),
        .package(url: "https://github.com/groue/GRDB.swift.git", from: "7.11.1"),
        .package(url: "https://github.com/modelcontextprotocol/swift-sdk.git", from: "0.11.0"),
        .package(url: "https://github.com/hummingbird-project/hummingbird.git", exact: "2.27.1", traits: ["FullFoundation"]),
        .package(url: "https://github.com/apple/swift-http-types.git", from: "1.8.0")
    ],
    targets: [
        .executableTarget(name: "ScannerHTTP", dependencies: [.product(name: "HummingbirdCore", package: "hummingbird"), "ScannerHTTPSupport", "ScannerAPI", "ScannerTransport", .product(name: "ArgumentParser", package: "swift-argument-parser"), .product(name: "Hummingbird", package: "hummingbird")]),
        .target(name: "ScannerHTTPSupport", dependencies: ["ScannerAPI", "ScannerTransport", .product(name: "HTTPTypes", package: "swift-http-types"), .product(name: "Hummingbird", package: "hummingbird")]),
        .testTarget(name: "ScannerHTTPTests", dependencies: [.product(name: "HTTPTypes", package: "swift-http-types"), "ScannerHTTPSupport", "ScannerAPI", "ScannerCore", "ScannerTransport", "ScannerServiceSupport", .product(name: "HummingbirdTesting", package: "hummingbird")]),
        .executableTarget(name: "ScannerCLI", dependencies: ["ScannerAPI", "ScannerTransport", .product(name: "ArgumentParser", package: "swift-argument-parser")]),
        .executableTarget(name: "ScannerService", dependencies: ["ScannerAPI", "ScannerCore", "ScannerPersistence", "ScannerTransport", "ScannerServiceSupport", .product(name: "ArgumentParser", package: "swift-argument-parser")]),
        .executableTarget(name: "ScannerMCP", dependencies: ["ScannerMCPSupport", "ScannerTransport", .product(name: "ArgumentParser", package: "swift-argument-parser")]),
        .target(name: "ScannerMCPSupport", dependencies: ["ScannerAPI", "ScannerTransport", .product(name: "MCP", package: "swift-sdk")]),
        .target(name: "ScannerAPI"),
        .target(name: "ScannerCore", dependencies: ["ScannerAPI", "ScannerPersistence"]),
        .target(name: "ScannerPersistence", dependencies: ["ScannerAPI", .product(name: "GRDB", package: "GRDB.swift")]),
        .target(name: "ScannerTransport", dependencies: ["ScannerAPI"]),
        .target(name: "ScannerServiceSupport", dependencies: ["ScannerCore", "ScannerAPI"]),
        .testTarget(name: "ScannerAPITests", dependencies: ["ScannerCore", "ScannerServiceSupport", "ScannerTransport", "ScannerAPI"]),
        .testTarget(name: "ScannerCLITests", dependencies: ["ScannerCLI", "ScannerAPI"]),
        .testTarget(name: "ScannerCoreTests", dependencies: ["ScannerCore", "ScannerAPI", "ScannerPersistence"]),
        .testTarget(name: "ScannerServiceTests", dependencies: ["ScannerCore", "ScannerAPI", "ScannerPersistence", "ScannerTransport", "ScannerServiceSupport"]),
        .testTarget(name: "ScannerMCPTests", dependencies: ["ScannerMCP", "ScannerMCPSupport", "ScannerCore", "ScannerAPI", "ScannerPersistence", "ScannerTransport", "ScannerServiceSupport", .product(name: "MCP", package: "swift-sdk")]),
        .testTarget(name: "ScannerPersistenceTests", dependencies: ["ScannerPersistence", "ScannerAPI"])
    ]
)
