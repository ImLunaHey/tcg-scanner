import Foundation
import CryptoKit
import Testing
import ScannerAPI
import ScannerClient
import ScannerCore
import ScannerServiceSupport
import ScannerTransport

private actor DeviceBackend: ScannerBackend {
    func devices() -> [Device] { [Device(id: "mock", name: "Mock")] }
    func inspect(deviceID: String) -> Inspection { Inspection(device: Device(id: deviceID, name: "Mock")) }
}
@Test @MainActor func nativeClientUsesSharedServiceContracts() async throws {
    let path = "/tmp/scanner-client-\(UUID().uuidString)/s.sock"
    let coordinator = try ScannerCoordinator(backend: DeviceBackend())
    let router = DiagnosticRouter(coordinator: coordinator)
    let server = UnixServer(path: path, handler: router.respond)
    try server.start()
    defer { server.stopAccepting(); try? FileManager.default.removeItem(atPath: (path as NSString).deletingLastPathComponent) }
    let client = ScannerClient(path: path)
    #expect(try await client.devices().first?.id == "mock")
    let profile = ScanProfile(name: "Documents")
    #expect(try await client.save(profile, creating: true) == profile)
    #expect(try await client.profiles().count == 1)
    var edited = profile; edited.duplex = true
    #expect(try await client.save(edited, creating: false).duplex)
    try await client.deleteProfile(profile.id)
    #expect(try await client.profiles().isEmpty)
    await #expect(throws: ScannerError.self) { try await client.job(UUID()) }
}
@Test func clientRejectsMismatchedResponses() async throws {
    let client = ScannerClient { _ in RPCResponse(id: .string("wrong"), result: .object([:])) }
    await #expect(throws: ScannerError.self) { try await client.status() }
}

@Test func verifiedExportsRejectCorruptionAndExistingDestinations() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
    defer { try? FileManager.default.removeItem(at: root) }
    let bytes = Data(repeating: 17, count: 300_000)
    let job = UUID()
    let digest = SHA256.hash(data: bytes).map { String(format: "%02x", $0) }.joined()
    let image = OriginalImage(sequence: 1, relativePath: "originals/page-000001.png", bytes: bytes.count, sha256: digest, width: 10, height: 10, imageCount: 1, uniformType: "public.png")
    let metadata = ImageMetadata(jobID: job, image: image)
    func client(corrupt: Bool) -> ScannerClient {
        ScannerClient { request in
            if request.method == "v1.images.get" { return RPCResponse(id: request.id, result: try .wrapping(metadata)) }
            guard case .object(let params) = request.params, case .number(let n) = params["offset"] else { throw ScannerError("test", "Missing offset") }
            let offset = Int(n); let chunk = bytes.subdata(in: offset..<min(offset + 262144, bytes.count))
            let hash = corrupt ? "bad" : SHA256.hash(data: chunk).map { String(format: "%02x", $0) }.joined()
            return RPCResponse(id: request.id, result: try .wrapping(ImageChunk(metadata: metadata, offset: offset, data: chunk, sha256: hash)))
        }
    }
    let output = root.appendingPathComponent("download.png")
    try await client(corrupt: false).export(jobID: job, sequence: 1, to: output)
    #expect(try Data(contentsOf: output) == bytes)
    await #expect(throws: (any Error).self) { try await client(corrupt: false).export(jobID: job, sequence: 1, to: output) }
    #expect(try Data(contentsOf: output) == bytes)
    let failed = root.appendingPathComponent("failed.png")
    await #expect(throws: ScannerError.self) { try await client(corrupt: true).export(jobID: job, sequence: 1, to: failed) }
    #expect(!FileManager.default.fileExists(atPath: failed.path))
    #expect(try FileManager.default.contentsOfDirectory(atPath: root.path) == ["download.png"])
}
