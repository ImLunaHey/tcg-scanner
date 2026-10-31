import Foundation
import Testing
import ScannerAPI
@testable import ScannerCore

private actor ScanMock: ScannerBackend {
    var calls = 0
    var hold = true
    func release() { hold = false }
    func devices() -> [Device] { [Device(id: "mock", name: "Mock")] }
    func inspect(deviceID: String) -> Inspection { Inspection(device: Device(id: deviceID, name: "Mock")) }
    func capture(deviceID: String, request: ScanRequest, store: ScanStorage) async throws -> CaptureOutcome {
        calls += 1
        try await store.markScanning(settings: [:])
        let file = store.incomingDirectory.appendingPathComponent("mock.png")
        try makeTestPNG(at: file)
        try await store.preserve(file, sequence: 1)
        do {
            while hold { try await Task.sleep(for: .milliseconds(5)) }
            return CaptureOutcome()
        } catch is CancellationError { return CaptureOutcome(cancelled: true) }
    }
}

@Test func scanOwnershipIdempotencyCancellationAndOriginalRetention() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let backend = ScanMock()
    let coordinator = try ScannerCoordinator(backend: backend, storageRoot: root)
    let request = ScanRequest(format: .png)
    let first = try await coordinator.startScan(request)
    #expect(try await coordinator.startScan(request).id == first.id)
    var changed = request; changed.duplex = true
    await #expect(throws: ScannerError.self) { try await coordinator.startScan(changed) }
    await #expect(throws: ScannerError.self) { try await coordinator.inspect(deviceID: nil) }
    let queued = try await coordinator.startScan(ScanRequest())
    #expect(queued.state == .queued)
    #expect(try await coordinator.cancelScan(queued.id).state == .cancelled)
    while (try await coordinator.scanStatus(first.id)).images.isEmpty { try await Task.sleep(for: .milliseconds(5)) }
    _ = try await coordinator.cancelScan(first.id)
    let deadline = ContinuousClock.now + .seconds(2)
    while !(try await coordinator.scanStatus(first.id)).state.terminal && ContinuousClock.now < deadline { await Task.yield() }
    let cancelled = try await coordinator.scanStatus(first.id)
    #expect(cancelled.state == .cancelled)
    #expect(cancelled.images.count == 1)
    #expect(cancelled.sessionClosed)
    #expect(await backend.calls == 1)
    await backend.release()
    let second = try await coordinator.startScan(ScanRequest(format: .png))
    while !(try await coordinator.scanStatus(second.id)).state.terminal { try await Task.sleep(for: .milliseconds(5)) }
    #expect(try await coordinator.scanStatus(second.id).state == .completed)
}

@Test func rejectsInvalidScanBeforeHardwareAndShutdownStopsNewScans() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let backend = ScanMock()
    let coordinator = try ScannerCoordinator(backend: backend, storageRoot: root)
    await #expect(throws: ScannerError.self) { try await coordinator.startScan(ScanRequest(dpi: -1)) }
    await #expect(throws: ScannerError.self) { try await coordinator.startScan(ScanRequest(outputDirectory: "../escape")) }
    #expect(await backend.calls == 0)
    await coordinator.shutdownScans()
    await #expect(throws: ScannerError.self) { try await coordinator.startScan(ScanRequest()) }
}
