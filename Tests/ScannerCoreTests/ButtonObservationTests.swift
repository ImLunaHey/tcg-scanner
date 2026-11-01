import Foundation
import Testing
import ScannerAPI
@testable import ScannerCore

private actor ButtonBackend: ScannerBackend {
    var observing = false
    var closed = false
    func devices() -> [Device] { [Device(id: "button-test", name: "Button fixture")] }
    func inspect(deviceID: String) throws -> Inspection { throw ScannerError("test", "Unexpected hardware inspection") }
    func observeButton(deviceID: String, seconds: Int) async throws {
        observing = true
        defer { observing = false; closed = true }
        try await Task.sleep(for: .seconds(seconds))
    }
}

@Test func buttonObservationReservesHardwareWithoutCreatingJobsAndCanBeStopped() async throws {
    let backend = ButtonBackend()
    let coordinator = try ScannerCoordinator(backend: backend)
    await #expect(throws: ScannerError.self) { try await coordinator.startButtonObservation(seconds: 301) }
    try await coordinator.startButtonObservation(seconds: 30)
    #expect(await coordinator.status().paused)
    await #expect(throws: ScannerError.self) { try await coordinator.startButtonObservation(seconds: 30) }
    await #expect(throws: ScannerError.self) { try await coordinator.startScan(ScanRequest()) }
    await #expect(throws: ScannerError.self) { try await coordinator.inspect(deviceID: nil) }
    let deadline = ContinuousClock.now + .seconds(2)
    while !(await backend.observing), ContinuousClock.now < deadline { try await Task.sleep(for: .milliseconds(10)) }
    #expect(await backend.observing)
    await coordinator.stopButtonObservation()
    #expect(await backend.closed)
    #expect(!(await coordinator.status().paused))
    #expect(try await coordinator.jobs().jobs.isEmpty)
}

@Test func buttonObservationClosesOnSleepAndShutdown() async throws {
    for sleep in [false, true] {
        let backend = ButtonBackend()
        let coordinator = try ScannerCoordinator(backend: backend)
        try await coordinator.startButtonObservation(seconds: 30)
        let deadline = ContinuousClock.now + .seconds(2)
        while !(await backend.observing), ContinuousClock.now < deadline { try await Task.sleep(for: .milliseconds(10)) }
        if sleep { await coordinator.prepareForSleep() } else { await coordinator.shutdownScans() }
        #expect(await backend.closed)
        #expect(!(await backend.observing))
        #expect(try await coordinator.jobs().jobs.isEmpty)
    }
}
