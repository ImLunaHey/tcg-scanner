import Foundation
import Testing
import ScannerAPI
import ScannerPersistence
@testable import ScannerCore

private actor FeedMock: ScannerBackend {
    var calls = 0
    var count = 2
    var hold = false
    func configure(count: Int, hold: Bool = false) { self.count = count; self.hold = hold }
    func devices() -> [Device] { [Device(id: "feed", name: "Feed mock")] }
    func inspect(deviceID: String) -> Inspection { Inspection(device: Device(id: deviceID, name: "Feed mock")) }
    func capture(deviceID: String, request: ScanRequest, store: ScanStorage) async throws -> CaptureOutcome {
        calls += 1
        try await store.markScanning(settings: ["dpi": .number(Double(request.dpi))])
        for n in 0..<count {
            let file = store.incomingDirectory.appendingPathComponent("image-\(n).png")
            try makeTestPNG(at: file); try await store.preserve(file, sequence: n + 1)
        }
        do {
            while hold { try await Task.sleep(for: .milliseconds(5)) }
            return CaptureOutcome()
        } catch is CancellationError { return CaptureOutcome(cancelled: true) }
    }
}
private func finished(_ service: ScannerCoordinator, _ id: UUID) async throws -> ScanRecord {
    let end = ContinuousClock.now + .seconds(10)
    while ContinuousClock.now < end {
        let record = try await service.scanStatus(id)
        if record.state.terminal { return record }
        try await Task.sleep(for: .milliseconds(5))
    }
    throw ScannerError("timeout", "Mock feed did not complete.")
}
@Test func continuingBatchPreservesNumbersMetadataAndIdempotencyAcrossRestart() async throws {
    let root = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let backend = FeedMock(), db = try ScannerDatabase()
    let service = try ScannerCoordinator(backend: backend, storageRoot: root, database: db)
    let first = try await service.startScan(ScanRequest(duplex: true, prepareCards: true, overscan: true))
    var old = try await finished(service, first.id)
    let identity = CardIdentification(request: IdentificationRequest(jobID: old.id, sequence: 2), sourceSHA256: old.images[1].sha256)
    old.cardIdentifications = [identity]; try db.save(old)
    let bytes = try old.images.map { try Data(contentsOf: root.appendingPathComponent("B-001/" + $0.relativePath)) }
    let retryID = UUID(), batchID = old.id
    async let a = service.continueBatch(batchID, requestID: retryID)
    async let b = service.continueBatch(batchID, requestID: retryID)
    let replies = try await [a,b]
    #expect(replies.allSatisfy { $0.id == old.id && $0.batchNumber == 1 })
    let result = try await finished(service, old.id)
    #expect(result.state == .completed && result.images.map(\.sequence) == [1,2,3,4])
    #expect(result.request == old.request && result.createdAt == old.createdAt && result.startedAt == old.startedAt)
    #expect(result.cardIdentifications?.first?.request.id == identity.request.id)
    #expect(result.processedImages?.filter { $0.request.sequence <= 2 }.map(\.id) == old.processedImages?.map(\.id))
    #expect(result.warnings.filter { $0.contains("image 1:") || $0.contains("image 2:") } == old.warnings.filter { $0.contains("image 1:") || $0.contains("image 2:") })
    #expect(result.continuations?.last?.imagesSaved == 2 && result.continuations?.last?.firstSequence == 3)
    #expect(result.continuations?.last?.state == .completed)
    #expect(await backend.calls == 2)
    for (i, image) in old.images.enumerated() { #expect(try Data(contentsOf: root.appendingPathComponent("B-001/" + image.relativePath)) == bytes[i]) }
    let restarted = try ScannerCoordinator(backend: backend, storageRoot: root, database: db)
    #expect(try await restarted.continueBatch(old.id, requestID: retryID).images.count == 4)
    #expect(await backend.calls == 2)
    await #expect(throws: ScannerError.self) { try await restarted.continueBatch(UUID(), requestID: retryID) }
    await #expect(throws: ScannerError.self) { try await restarted.startScan(ScanRequest(requestID: retryID)) }
    await #expect(throws: ScannerError.self) { try await restarted.continueBatch(old.id, requestID: old.id) }
    let next = try await restarted.startScan(ScanRequest())
    #expect(next.batchNumber == 2)
    _ = try await finished(restarted, next.id)
}
@Test func continuationCancellationAndIncompletePairsRetainEarlierImages() async throws {
    let root = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let backend = FeedMock(), db = try ScannerDatabase()
    let service = try ScannerCoordinator(backend: backend, storageRoot: root, database: db)
    let first = try await service.startScan(ScanRequest(duplex: true))
    let old = try await finished(service, first.id)
    await backend.configure(count: 1, hold: true)
    _ = try await service.continueBatch(old.id, requestID: UUID())
    let deadline = ContinuousClock.now + .seconds(5)
    while (try await service.scanStatus(old.id)).images.count < 3 && ContinuousClock.now < deadline { try await Task.sleep(for: .milliseconds(5)) }
    await #expect(throws: ScannerError.self) { try await service.continueBatch(old.id, requestID: UUID()) }
    _ = try await service.cancelScan(old.id)
    let result = try await finished(service, old.id)
    #expect(result.images.count == 3 && result.state == .cancelled)
    #expect(result.continuations?.last?.state == .cancelled && result.continuations?.last?.imagesSaved == 1)
    await #expect(throws: ScannerError.self) { try await service.continueBatch(old.id, requestID: UUID()) }
}
@Test func continuationRecoveryRejectsPriorPassManifestAndDoesNotFeed() async throws {
    let root = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let backend = FeedMock(), db = try ScannerDatabase()
    let service = try ScannerCoordinator(backend: backend, storageRoot: root, database: db)
    let first = try await service.startScan(ScanRequest(duplex: true))
    let old = try await finished(service, first.id)
    let key = UUID()
    let pending = try ScanStorage(root: root, continuing: old, requestID: key)
    try db.beginContinuation(pending.initialRecord, requestID: key)
    #expect(try db.recoverInterruptedJobs(storageRoot: root) == 1)
    let recovered = try #require(try db.job(old.id))
    #expect(recovered.state == .failed && recovered.images.count == 2)
    #expect(recovered.continuations?.last?.requestID == key && recovered.continuations?.last?.state == .failed)
    #expect(try await service.continueBatch(old.id, requestID: key).state == .failed)
    #expect(await backend.calls == 1)
}
@Test func continuationOffsetsBeyondOneFeedAndDetectsEmptyFeed() async throws {
    let root = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let backend = FeedMock()
    let service = try ScannerCoordinator(backend: backend, storageRoot: root)
    await backend.configure(count: 100)
    let first = try await service.startScan(ScanRequest(duplex: true))
    _ = try await finished(service, first.id)
    await backend.configure(count: 2)
    _ = try await service.continueBatch(first.id, requestID: UUID())
    let result = try await finished(service, first.id)
    #expect(result.images.count == 102 && result.images.last?.sequence == 102)
    try IdentificationRequest(jobID: first.id, sequence: 102).validate()
    try ImageEditRequest(jobID: first.id, sequence: 102).validate()
    await backend.configure(count: 0)
    _ = try await service.continueBatch(first.id, requestID: UUID())
    let empty = try await finished(service, first.id)
    #expect(empty.state == .failed && empty.errors.contains { $0.kind == "noImages" })
    #expect(empty.images.count == 102)
}
