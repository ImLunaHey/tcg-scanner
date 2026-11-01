import Foundation
import Testing
import ScannerAPI
@testable import ScannerCore

@Test func scanButtonTriggerIgnoresOtherButtonsAndEmptyFeeder() {
    var trigger = ScanButtonTrigger()
    trigger.receive(isScan: false, documentLoaded: true)
    #expect(!trigger.accepted)
    trigger.receive(isScan: true, documentLoaded: false)
    #expect(!trigger.accepted)
    trigger.receive(isScan: true, documentLoaded: true)
    #expect(trigger.accepted)
    trigger.receive(isScan: false, documentLoaded: false)
    #expect(trigger.accepted) // One latched event; never a press counter/queue.
}

private actor AutomaticButtonBackend: ScannerBackend {
    var listening = false
    var pressed = false
    var connected = true
    var failClosure = false
    var requests: [ScanRequest] = []
    var inspections = 0
    var overlappingSessions = false
    func devices() -> [Device] { connected ? [Device(id: "button", name: "Button mock")] : [] }
    func press() { if listening { pressed = true } }
    func disconnect() { connected = false }
    func reconnect() { connected = true }
    func breakClosure() { failClosure = true }
    func waitForScanButton(deviceID: String) async throws {
        if listening { overlappingSessions = true }
        listening = true; pressed = false
        defer { listening = false }
        do {
            while !pressed {
                guard connected else { throw ScannerError("disconnected", "Unplugged") }
                try await Task.sleep(for: .milliseconds(5))
            }
        } catch {
            // Simulate asynchronous session cleanup even when the caller cancels.
            await Task { try? await Task.sleep(for: .milliseconds(20)) }.value
            if failClosure { throw ScannerError("cleanup", "Closure unconfirmed") }
            throw error
        }
    }
    func inspect(deviceID: String) -> Inspection {
        if listening { overlappingSessions = true }
        inspections += 1
        return Inspection(device: Device(id: deviceID, name: "Button mock"))
    }
    func capture(deviceID: String, request: ScanRequest, store: ScanStorage) async throws -> CaptureOutcome {
        if listening { overlappingSessions = true }
        requests.append(request)
        try await store.markScanning(settings: [:])
        do { try await Task.sleep(for: .seconds(30)) }
        catch { return CaptureOutcome(cancelled: true) }
        return CaptureOutcome()
    }
}

private func eventually(_ predicate: () async throws -> Bool) async throws {
    let deadline = ContinuousClock.now + .seconds(3)
    while !(try await predicate()) {
        guard ContinuousClock.now < deadline else { throw ScannerError("testTimeout", "Button state did not settle") }
        try await Task.sleep(for: .milliseconds(10))
    }
}

@Test func physicalButtonCreatesExactlyOneNormalCardJobAndDoesNotQueueBusyPresses() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let backend = AutomaticButtonBackend()
    let coordinator = try ScannerCoordinator(backend: backend, storageRoot: root, scanButtonDeviceID: "button")
    await coordinator.maintainScanButton()
    try await eventually { await backend.listening }
    #expect(!(await coordinator.status().paused))
    #expect(try await coordinator.jobs().jobs.isEmpty)
    for _ in 0..<10 { await backend.press() }
    try await eventually { await backend.requests.count == 1 }
    let request = try #require(await backend.requests.first)
    #expect(request.overscan == true)
    #expect(request.deviceID == "button" && request.dpi == 300 && request.duplex && request.format == .png && request.prepareCards == true)
    for _ in 0..<10 { await coordinator.maintainScanButton(); await backend.press() }
    #expect(try await coordinator.jobs().jobs.count == 1)
    #expect(!(await backend.overlappingSessions))
    _ = try await coordinator.cancelScan(request.requestID)
    try await eventually { try await coordinator.scanStatus(request.requestID).state.terminal }
    try await Task.sleep(for: .seconds(5.1))
    await coordinator.maintainScanButton()
    try await eventually { await backend.listening }
    await backend.press()
    try await eventually { await backend.requests.count == 2 }
    #expect(try await coordinator.jobs().jobs.count == 2)
    await coordinator.shutdownScans()
    #expect(try await coordinator.scanStatus(request.requestID).state == .cancelled)
}

@Test func ordinaryClientsTakeOwnershipFromAutomaticButtonListener() async throws {
    for inspect in [false, true] {
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let backend = AutomaticButtonBackend()
        let coordinator = try ScannerCoordinator(backend: backend, storageRoot: root, scanButtonDeviceID: "button")
        await coordinator.maintainScanButton()
        try await eventually { await backend.listening }
        if inspect { _ = try await coordinator.inspect(deviceID: "button") }
        else {
            _ = try await coordinator.startScan(ScanRequest())
            try await eventually { await backend.requests.count == 1 }
        }
        #expect(!(await backend.listening))
        #expect(!(await backend.overlappingSessions))
        await coordinator.shutdownScans()
    }
}

@Test func automaticButtonStopsForSleepAndShutdownWithoutCreatingJobs() async throws {
    for sleep in [false, true] {
        let backend = AutomaticButtonBackend()
        let coordinator = try ScannerCoordinator(backend: backend, scanButtonDeviceID: "button")
        await coordinator.maintainScanButton()
        try await eventually { await backend.listening }
        if sleep { await coordinator.prepareForSleep() } else { await coordinator.shutdownScans() }
        await coordinator.maintainScanButton()
        #expect(!(await backend.listening))
        #expect(try await coordinator.jobs().jobs.isEmpty)
        if sleep {
            try await coordinator.resumeAfterWake()
            try await Task.sleep(for: .seconds(5.1))
            await coordinator.maintainScanButton()
            try await eventually { await backend.listening }
            #expect(try await coordinator.jobs().jobs.isEmpty)
            await coordinator.shutdownScans()
        }
    }
}

@Test func failedButtonSessionClosurePreventsClientCapture() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let backend = AutomaticButtonBackend()
    let coordinator = try ScannerCoordinator(backend: backend, storageRoot: root, scanButtonDeviceID: "button")
    await coordinator.maintainScanButton()
    try await eventually { await backend.listening }
    await backend.breakClosure()
    let record = try await coordinator.startScan(ScanRequest())
    try await eventually { try await coordinator.scanStatus(record.id).state.terminal }
    #expect(try await coordinator.scanStatus(record.id).state == .failed)
    #expect(await coordinator.requiresRestart)
    #expect(await backend.requests.isEmpty)
    await coordinator.shutdownScans()
}

@Test func automaticButtonReconnectsWithoutScanningAndRequiresExplicitConfiguration() async throws {
    let backend = AutomaticButtonBackend()
    let disabled = try ScannerCoordinator(backend: backend)
    await disabled.maintainScanButton()
    #expect(!(await backend.listening))
    let coordinator = try ScannerCoordinator(backend: backend, scanButtonDeviceID: "button")
    await coordinator.maintainScanButton()
    try await eventually { await backend.listening }
    await backend.disconnect()
    try await eventually { !(await backend.listening) }
    await backend.reconnect()
    await coordinator.maintainScanButton()
    #expect(!(await backend.listening)) // Backoff/debounce remains in force.
    try await Task.sleep(for: .seconds(5.1))
    await coordinator.maintainScanButton()
    try await eventually { await backend.listening }
    #expect(try await coordinator.jobs().jobs.isEmpty)
    await coordinator.shutdownScans()
}
