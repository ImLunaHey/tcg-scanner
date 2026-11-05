import Foundation
import ImageCaptureCore
import Testing
import ScannerAPI
@testable import ScannerCore

private actor ReconnectingBackend: ScannerBackend {
    var connected = true
    var removed = false
    var waiting = true
    var captures = 0
    var listenerStarted = false
    var listenerFinished = false
    func devices() -> [Device] { connected ? [Device(id: "reconnect", name: "Reconnect mock")] : [] }
    func inspect(deviceID: String) -> Inspection { Inspection(device: Device(id: deviceID, name: "Reconnect mock")) }
    func unplug() { connected = false; removed = true; waiting = false }
    func powerOn() { connected = true }
    func recoverAfterDisconnect() -> Bool { removed }
    func capture(deviceID: String, request: ScanRequest, store: ScanStorage) async throws -> CaptureOutcome {
        captures += 1
        try await store.markScanning(settings: [:])
        let file = store.incomingDirectory.appendingPathComponent("received.png")
        try makeTestPNG(at: file)
        try await store.preserve(file, sequence: 1)
        while waiting { try await Task.sleep(for: .milliseconds(5)) }
        return connected ? CaptureOutcome() : CaptureOutcome(errors: [ScannerError("disconnected", "Power off")], sessionClosed: false)
    }
    func waitForScanButton(deviceID: String) async throws {
        listenerStarted = true
        defer { listenerFinished = true }
        while connected { try await Task.sleep(for: .milliseconds(5)) }
        // Exercise delayed/failed driver cleanup rather than a clean disconnect.
        throw ScannerError("cleanup", "Close callback missing after unplug")
    }
}

private func eventually(isolation: isolated (any Actor)? = #isolation, _ predicate: () async throws -> Bool) async throws {
    let deadline = ContinuousClock.now + .seconds(5)
    while try await !predicate() {
        guard .now < deadline else { throw ScannerError("testTimeout", "Recovery did not complete") }
        try await Task.sleep(for: .milliseconds(5))
    }
}

@Test func powerCyclePreservesImagesFailsQueueAndAcceptsOnlyFreshJobs() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let backend = ReconnectingBackend()
    let service = try ScannerCoordinator(backend: backend, storageRoot: root)
    let first = try await service.startScan(ScanRequest(format: .png))
    try await eventually { try await service.scanStatus(first.id).images.count == 1 }
    let queued = try await service.startScan(ScanRequest(format: .png))
    await backend.unplug()
    try await eventually { await service.status().activeJobID == nil }
    #expect(await service.requiresRestart)
    let failed = try await service.scanStatus(first.id)
    #expect(failed.state == .failed && failed.images.count == 1)
    #expect(FileManager.default.fileExists(atPath: URL(fileURLWithPath: failed.outputDirectory).appendingPathComponent("C-001.png").path))
    #expect(try await service.scanStatus(queued.id).state == .failed)
    #expect(try await service.devices().isEmpty)
    #expect(await !service.requiresRestart)
    await backend.powerOn()
    #expect(try await service.devices().count == 1)
    #expect(await backend.captures == 1)
    let fresh = try await service.startScan(ScanRequest(format: .png))
    try await eventually { try await service.scanStatus(fresh.id).state == .completed }
    #expect(await backend.captures == 2)
    await service.shutdownScans()
}

@Test func idleButtonCleanupRecoversAfterPowerCycleWithoutStartingBatch() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let backend = ReconnectingBackend()
    let service = try ScannerCoordinator(backend: backend, storageRoot: root, scanButtonDeviceID: "reconnect")
    await service.maintainScanButton()
    try await eventually { await backend.listenerStarted }
    await backend.unplug()
    try await eventually { await service.automaticButtonTask == nil }
    #expect(await service.requiresRestart)
    await backend.powerOn()
    _ = try await service.devices()
    #expect(await !service.requiresRestart)
    #expect(await backend.captures == 0)
    await service.shutdownScans()
}

private final class SilentScanner: ICScannerDevice {
    var openRequests = 0
    var closeRequests = 0
    override func requestOpenSession() { openRequests += 1 }
    override func requestCloseSession() { closeRequests += 1 }
}

@Test @MainActor func removedConnectionDoesNotWaitForImpossibleCloseCallback() async throws {
    let scanner = SilentScanner()
    let other = ICScannerDevice()
    let connection = ImageCaptureConnection(scanner: scanner, log: { _ in })
    let opening = Task { try await connection.connect() }
    try await eventually { scanner.openRequests == 1 }
    connection.didRemove(other)
    #expect(!connection.removed)
    connection.didRemove(scanner)
    #expect(connection.removed)
    await #expect(throws: ScannerError.self) { try await opening.value }
    let errors = await connection.close()
    #expect(scanner.closeRequests == 0)
    #expect(errors.map(\.kind) == ["disconnected"])
    #expect(!connection.sessionClosed) // Do not invent a driver acknowledgement.
}

@Test @MainActor func backendQuarantineRequiresRemovalOfTheExactScannerSession() {
    let backend = ImageCaptureBackend()
    let scanner = ICScannerDevice()
    let other = ICScannerDevice()
    let connection = ImageCaptureConnection(scanner: scanner, log: { _ in })
    backend.quarantine(connection)
    #expect(!backend.recoverAfterDisconnect())
    let browser = ICDeviceBrowser()
    backend.deviceBrowser(browser, didRemove: other, moreGoing: false)
    #expect(!backend.recoverAfterDisconnect())
    backend.deviceBrowser(browser, didRemove: scanner, moreGoing: false)
    #expect(backend.recoverAfterDisconnect())
    #expect(connection.removed)
    // Removal during capture/cleanup may precede quarantine registration.
    let second = ImageCaptureConnection(scanner: other, log: { _ in })
    second.didRemove(other)
    backend.quarantine(second)
    #expect(backend.recoverAfterDisconnect())
}
