// swift-tools-version: 6.2
import PackageDescription
let package = Package(name: "ScannerVisionProbe", platforms: [.macOS(.v14)], products: [.executable(name: "Probe", targets: ["Probe"])], dependencies: [
.package(url: "https://github.com/ml-explore/mlx-swift-lm", exact: "3.32.3", traits: []),
.package(url: "https://github.com/huggingface/swift-transformers", from: "1.3.0")
], targets: [.executableTarget(name: "Probe", dependencies: [
.product(name: "MLXVLM", package: "mlx-swift-lm"),
.product(name: "MLXLMCommon", package: "mlx-swift-lm"),
.product(name: "Tokenizers", package: "swift-transformers")])])
