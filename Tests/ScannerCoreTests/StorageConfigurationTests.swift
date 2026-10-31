import Foundation
import Testing
import ScannerAPI
@testable import ScannerCore

@Test func requiredSMBPolicyRejectsLocalFallbackBeforeCreatingBatch() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    let policy = StorageConfiguration(storageRoot: root.path, requiredSMBShare: "smb://lake/scans")
    #expect(throws: ScannerError.self) { try policy.validate() }
    #expect(throws: ScannerError.self) { try ScanStorage(root: root, device: Device(id: "mock", name: "Mock"), request: ScanRequest(), validateStorage: policy.validate) }
    #expect(!FileManager.default.fileExists(atPath: root.path))
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
    defer { try? FileManager.default.removeItem(at: root) }
    #expect(throws: ScannerError.self) { try policy.validate() }
    #expect(try FileManager.default.contentsOfDirectory(atPath: root.path).isEmpty)
}
@Test func storageConfigurationRejectsCredentialsAndLoadsPersistentLocation() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: false)
    defer { try? FileManager.default.removeItem(at: root) }
    for share in ["smb://user:password@lake/scans", "https://lake/scans", "smb://lake", "smb://lake/scans/subfolder"] {
        #expect(throws: ScannerError.self) { try StorageConfiguration(storageRoot: root.path, requiredSMBShare: share).validate() }
    }
    let file = root.appendingPathComponent("storage.json")
    let policy = StorageConfiguration(storageRoot: "/Volumes/scans", requiredSMBShare: "smb://lake/scans")
    try JSONEncoder().encode(policy).write(to: file)
    let loaded = try StorageConfiguration.load(from: file)
    #expect(loaded.storageRoot == policy.storageRoot && loaded.requiredSMBShare == policy.requiredSMBShare)
}
@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_TEST_SMB_ROOT"] != nil))
func mountedSMBStoresFlatBatchWithNoHardwareAccess() async throws {
    let mount = try #require(ProcessInfo.processInfo.environment["SCANNER_TEST_SMB_ROOT"])
    let policy = StorageConfiguration(storageRoot: mount, requiredSMBShare: "smb://lake/scans")
    try policy.validate()
    let root = URL(fileURLWithPath: mount).appendingPathComponent(".scanner-verification-" + UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let request = ScanRequest(format: .png)
    let store = try ScanStorage(root: root, device: Device(id: "mock", name: "Mock"), request: request, validateStorage: policy.validate)
    let incoming = store.incomingDirectory.appendingPathComponent("fixture.png")
    try makeTestPNG(at: incoming)
    try await store.preserve(incoming, sequence: 1)
    try await store.finish(errors: [], cancelled: false, sessionClosed: true)
    let record = await store.snapshot()
    #expect(record.state == .completed)
    #expect(URL(fileURLWithPath: record.outputDirectory).lastPathComponent == "batch-" + request.requestID.uuidString.lowercased())
    #expect(record.images.first?.relativePath == "page-000001.png")
    #expect(!FileManager.default.fileExists(atPath: URL(fileURLWithPath: record.outputDirectory).appendingPathComponent("originals").path))
    #expect(!FileManager.default.fileExists(atPath: URL(fileURLWithPath: record.outputDirectory).appendingPathComponent("processed").path))
    #expect(try Data(contentsOf: incoming) == Data(contentsOf: URL(fileURLWithPath: record.outputDirectory).appendingPathComponent("page-000001.png")))
}
