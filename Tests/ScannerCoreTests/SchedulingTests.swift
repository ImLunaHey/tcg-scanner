import Foundation
import Testing
import ScannerAPI
import ScannerPersistence
@testable import ScannerCore

private actor QueueBackend: ScannerBackend {
    var connected = true
    var hold = true
    var entered: [ScanRequest] = []
    var active = 0
    var maxActive = 0
    func devices() async throws -> [Device] {
        try await Task.sleep(for: .milliseconds(2)) // Exercise submission reentrancy.
        return connected ? [Device(id: "queue", name: "Queue mock")] : []
    }
    func disconnect() { connected = false; hold = false }
    func release() { hold = false }
    func inspect(deviceID: String) -> Inspection { Inspection(device: Device(id: deviceID, name: "Queue mock")) }
    func capture(deviceID: String, request: ScanRequest, store: ScanStorage) async throws -> CaptureOutcome {
        active += 1; maxActive = max(maxActive, active); entered.append(request)
        defer { active -= 1 }
        guard connected else { return CaptureOutcome(errors: [ScannerError("disconnected", "Mock unplugged")]) }
        try await store.markScanning(settings: [:])
        let file = store.incomingDirectory.appendingPathComponent("page.png")
        try makeTestPNG(at: file)
        try await store.preserve(file, sequence: 1)
        do { while hold { try await Task.sleep(for: .milliseconds(5)) } }
        catch is CancellationError { return CaptureOutcome(cancelled: true) }
        guard connected else { return CaptureOutcome(errors: [ScannerError("disconnected", "Mock unplugged")]) }
        return CaptureOutcome()
    }
}
private func waitFor(_ coordinator: ScannerCoordinator, id: UUID, terminal: Bool = true) async throws -> ScanRecord {
    let deadline = ContinuousClock.now + .seconds(5)
    while true {
        let record = try await coordinator.scanStatus(id)
        if terminal ? record.state.terminal : record.state == .scanning && !record.images.isEmpty { return record }
        guard ContinuousClock.now < deadline else { throw ScannerError("testTimeout", "Job never reached expected state.") }
        try await Task.sleep(for: .milliseconds(5))
    }
}

@Test func concurrentSubmissionDeduplicatesAndQueueUsesProfileSnapshots() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let backend = QueueBackend()
    let db = try ScannerDatabase()
    let coordinator = try ScannerCoordinator(backend: backend, storageRoot: root, database: db)
    let request = ScanRequest(format: .png)
    let ids = try await withThrowingTaskGroup(of: UUID.self) { group in
        for _ in 0..<20 { group.addTask { try await coordinator.startScan(request).id } }
        var ids: [UUID] = []
        for try await id in group { ids.append(id) }
        return ids
    }
    #expect(Set(ids) == [request.requestID])
    _ = try await waitFor(coordinator, id: request.requestID, terminal: false)
    var profile = ScanProfile(name: "Duplex", dpi: 300, duplex: true, format: .png)
    try await coordinator.saveProfile(profile)
    let second = try await coordinator.startScan(ScanRequest(profileID: profile.id))
    #expect(second.state == .queued)
    profile.dpi = 600; profile.duplex = false
    try await coordinator.saveProfile(profile)
    try await coordinator.deleteProfile(profile.id)
    let third = try await coordinator.startScan(ScanRequest(format: .png))
    #expect(try await coordinator.cancelScan(third.id).state == .cancelled)
    #expect(await backend.entered.count == 1)
    await backend.release()
    #expect(try await waitFor(coordinator, id: request.requestID).state == .completed)
    let done = try await waitFor(coordinator, id: second.id)
    #expect(done.state == .completed)
    #expect(done.request.dpi == 300 && done.request.duplex)
    #expect(done.profile?.name == "Duplex")
    #expect(done.startedAt != nil && done.completedAt != nil)
    #expect(await backend.maxActive == 1)
    #expect(await backend.entered.count == 2)
    #expect(try db.jobs().jobs.count == 3)
    let events = try await coordinator.events(after: 0)
    let updates = try events.events.filter { $0.kind == "job.updated" }.map { try $0.payload.decode(JobSummary.self) }
    #expect(updates.contains { $0.id == second.id && $0.state == .queued })
    #expect(updates.contains { $0.id == second.id && $0.state == .processing })
    await coordinator.shutdownScans()
    let restarted = try ScannerCoordinator(backend: backend, storageRoot: root, database: db)
    #expect(try await restarted.startScan(request).state == .completed)
    #expect(await backend.entered.count == 2)
}

@Test func shutdownAndSleepCancelQueuedJobsWithoutFeedingThem() async throws {
    for sleep in [false, true] {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let backend = QueueBackend()
        let coordinator = try ScannerCoordinator(backend: backend, storageRoot: root)
        let active = try await coordinator.startScan(ScanRequest(format: .png))
        _ = try await waitFor(coordinator, id: active.id, terminal: false)
        let queued = try await coordinator.startScan(ScanRequest(format: .png))
        if sleep { await coordinator.prepareForSleep() } else { await coordinator.shutdownScans() }
        #expect(try await coordinator.scanStatus(active.id).state == .cancelled)
        #expect(try await coordinator.scanStatus(queued.id).state == .cancelled)
        #expect(await backend.entered.count == 1)
        await #expect(throws: ScannerError.self) { try await coordinator.startScan(ScanRequest()) }
        if sleep {
            try await coordinator.resumeAfterWake()
            await backend.release()
            let resumed = try await coordinator.startScan(ScanRequest(format: .png))
            #expect(try await waitFor(coordinator, id: resumed.id).state == .completed)
        }
    }
}

@Test func disconnectFailsActiveAndQueuedJobsAndDiscoveryReportsReconnect() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let backend = QueueBackend()
    let coordinator = try ScannerCoordinator(backend: backend, storageRoot: root)
    _ = try await coordinator.devices()
    let first = try await coordinator.startScan(ScanRequest(format: .png))
    _ = try await waitFor(coordinator, id: first.id, terminal: false)
    let second = try await coordinator.startScan(ScanRequest(format: .png))
    await backend.disconnect()
    #expect(try await coordinator.devices().isEmpty)
    #expect(try await waitFor(coordinator, id: first.id).errors.first?.kind == "disconnected")
    #expect(try await waitFor(coordinator, id: second.id).errors.first?.kind == "disconnected")
    #expect(try await coordinator.events(after: 0).events.contains { $0.kind == "device.disconnected" })
}

private actor UnclosedBackend: ScannerBackend {
    var calls = 0
    func devices() -> [Device] { [Device(id: "unclosed", name: "Unclosed mock")] }
    func inspect(deviceID: String) -> Inspection { Inspection(device: Device(id: deviceID, name: "Unclosed mock")) }
    func capture(deviceID: String, request: ScanRequest, store: ScanStorage) -> CaptureOutcome {
        calls += 1
        return CaptureOutcome(errors: [ScannerError("cleanup", "Simulated unconfirmed session closure")], sessionClosed: false)
    }
}
@Test func wakingCannotClearUnconfirmedHardwareOwnership() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let backend = UnclosedBackend()
    let coordinator = try ScannerCoordinator(backend: backend, storageRoot: root)
    let record = try await coordinator.startScan(ScanRequest())
    _ = try await waitFor(coordinator, id: record.id)
    try await coordinator.resumeAfterWake()
    #expect(await coordinator.status().paused)
    await #expect(throws: ScannerError.self) { try await coordinator.startScan(ScanRequest()) }
    #expect(await backend.calls == 1)
}

@Test func storageFailureStopsQueueAndRetainsReceivedFiles() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let backend = QueueBackend()
    let coordinator = try ScannerCoordinator(backend: backend, storageRoot: root)
    let first = try await coordinator.startScan(ScanRequest(format: .png))
    _ = try await waitFor(coordinator, id: first.id, terminal: false)
    let second = try await coordinator.startScan(ScanRequest(format: .png))
    // Simulate a storage location becoming unavailable without deleting any files.
    let moved = root.appendingPathComponent("moved-batch")
    try FileManager.default.moveItem(at: URL(fileURLWithPath: first.outputDirectory), to: moved)
    await backend.release()
    #expect(try await waitFor(coordinator, id: first.id).state == .failed)
    #expect(await coordinator.status().persistenceError != nil)
    #expect(try await coordinator.scanStatus(second.id).state == .queued)
    #expect(await backend.entered.count == 1)
    #expect(FileManager.default.fileExists(atPath: moved.appendingPathComponent("manifest.json").path))
    #expect(FileManager.default.fileExists(atPath: moved.appendingPathComponent("originals/page-000001.png").path))
    await coordinator.shutdownScans()
}
