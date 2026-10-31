import Foundation
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
