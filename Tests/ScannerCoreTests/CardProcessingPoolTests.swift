import Foundation
import Testing
import ScannerAPI
@testable import ScannerCore

private actor WorkProbe {
    var active = 0
    var maximum = 0
    var started: [Int] = []
    var finished: [Int] = []
    var hold = true
    func release() { hold = false }
    func visit(_ value: Int) async {
        active += 1; maximum = max(maximum, active); started.append(value)
        defer { active -= 1 }
        do { while hold { try await Task.sleep(for: .milliseconds(5)) } }
        catch { return }
        guard !Task.isCancelled else { return }
        finished.append(value)
    }
}
@Test func cardProcessingRunsConcurrentlyWithinItsMemoryBound() async throws {
    let probe = WorkProbe()
    let task = Task { await CardProcessingPool.run(Array(0..<10)) { await probe.visit($0) } }
    let deadline = ContinuousClock.now + .seconds(3)
    while await probe.active != 2, .now < deadline { try await Task.sleep(for: .milliseconds(5)) }
    #expect(await probe.started.sorted() == [0,1])
    await probe.release()
    await task.value
    #expect(await probe.maximum == 2)
    #expect(await probe.finished.sorted() == Array(0..<10))
}
@Test func cancellingCardProcessingDrainsWorkersWithoutStartingPendingCards() async throws {
    let probe = WorkProbe()
    let task = Task { await CardProcessingPool.run(Array(0..<10)) { await probe.visit($0) } }
    let deadline = ContinuousClock.now + .seconds(3)
    while await probe.active != 2, .now < deadline { try await Task.sleep(for: .milliseconds(5)) }
    task.cancel(); await task.value
    #expect(await probe.started.sorted() == [0,1])
    #expect(await probe.active == 0)
    #expect(await probe.finished.isEmpty)
}

@Test func progressKnowsTheFinalImageCountWhileProcessing() {
    let request = ScannerAPI.ScanRequest()
    var record = ScannerAPI.ScanRecord(device: ScannerAPI.Device(id: "fixture", name: "Fixture"), request: request, outputDirectory: "/tmp/fixture")
    record.state = .scanning
    #expect(record.progress.totalImages == nil)
    record.state = .processing
    #expect(record.progress.totalImages == 0)
}
