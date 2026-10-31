import CoreGraphics
import CryptoKit
import Foundation
import ImageIO
import Testing
import ScannerAPI
import ScannerCore
import ScannerPersistence
import ScannerRecognition
import ScannerServiceSupport

private struct NoHardware: ScannerBackend {
    func devices() -> [Device] { [] }
    func inspect(deviceID: String) throws -> Inspection { throw ScannerError("test", "Hardware must not be accessed.") }
}
private actor RecognitionStub: CardRecognizing {
    var calls = 0
    var fail: Bool
    init(fail: Bool = false) { self.fail = fail }
    func recognize(_ data: Data) async throws -> RecognitionEvidence {
        calls += 1
        try await Task.sleep(for: .milliseconds(200))
        if fail { throw ScannerError("catalogueUnavailable", "Offline fixture") }
        return RecognitionEvidence(lines: [RecognizedLine(text: "Furret", y: 0.9)], candidates: [
            CardCandidate(id: "swsh3-136", name: "Furret", localID: "136", setID: "swsh3", setName: "Darkness Ablaze", officialSetCount: 189, evidence: ["Fixture match"], availableFinishes: [.normal, .reverseHolo])])
    }
}
private func savedBatch(root: URL, database: ScannerDatabase) throws -> ScanRecord {
    let batch = root.appendingPathComponent("B-001")
    try FileManager.default.createDirectory(at: batch, withIntermediateDirectories: true)
    let context = try #require(CGContext(data: nil, width: 120, height: 80, bitsPerComponent: 8, bytesPerRow: 480,
        space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
    let bytes = NSMutableData()
    let encoder = try #require(CGImageDestinationCreateWithData(bytes, "public.png" as CFString, 1, nil))
    CGImageDestinationAddImage(encoder, try #require(context.makeImage()), nil)
    #expect(CGImageDestinationFinalize(encoder))
    let data = bytes as Data
    try data.write(to: batch.appendingPathComponent("C-001.png"))
    var record = ScanRecord(device: Device(id: "fixture", name: "Saved image"), request: ScanRequest(), outputDirectory: batch.path)
    record.state = .completed
    record.images = [OriginalImage(sequence: 1, relativePath: "C-001.png", bytes: data.count,
        sha256: SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined(), width: 120, height: 80, imageCount: 1, uniformType: "public.png")]
    try database.insert(record, submission: record.request)
    return record
}
private func completed(_ coordinator: ScannerCoordinator, job: UUID) async throws -> CardIdentification {
    let deadline = ContinuousClock.now + .seconds(5)
    while ContinuousClock.now < deadline {
        if let value = try await coordinator.cardIdentification(jobID: job, sequence: 1), value.state != .processing { return value }
        try await Task.sleep(for: .milliseconds(10))
    }
    throw ScannerError("test", "Recognition did not finish.")
}

@Test func identificationIsAsyncIdempotentReviewedAndPersistedWithoutHardware() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let database = try ScannerDatabase()
    let record = try savedBatch(root: root, database: database)
    let stub = RecognitionStub()
    let service = try ScannerCoordinator(backend: NoHardware(), storageRoot: root, database: database, cardRecognizer: stub)
    let request = IdentificationRequest(jobID: record.id, sequence: 1)
    let pending = try await service.identifyCard(request)
    #expect(pending.state == .processing)
    let again = try await service.identifyCard(request)
    #expect(again.request.id == pending.request.id)
    await #expect(throws: ScannerError.self) { try await service.identifyCard(IdentificationRequest(jobID: record.id, sequence: 1)) }
    // An edit finishing while OCR is in flight must not overwrite recognition metadata.
    let edit = try await service.editImage(ImageEditRequest(jobID: record.id, sequence: 1, quarterTurns: 1))
    let result = try await completed(service, job: record.id)
    #expect(result.state == .reviewRequired && result.selectedCardID == nil)
    #expect(result.finish == .unverified && result.condition == .unassessed)
    #expect(await stub.calls == 1)
    #expect(result.finishEstimate?.state == .uncertain)
    await #expect(throws: ScannerError.self) { try await service.reviewCard(jobID: record.id, sequence: 1, identificationID: request.id, cardID: "invented", finish: .holo, condition: .nearMint) }
    let reviewed = try await service.reviewCard(jobID: record.id, sequence: 1, identificationID: request.id, cardID: "swsh3-136", finish: .reverseHolo, condition: .lightlyPlayed)
    #expect(reviewed.state == .confirmed && reviewed.condition == .lightlyPlayed)
    let savedReview = try #require(await service.cardIdentification(jobID: record.id, sequence: 1))
    let estimated = try await service.estimateFinish(jobID: record.id, sequence: 1, identificationID: request.id)
    #expect(estimated.finish == .reverseHolo && estimated.condition == .lightlyPlayed)
    #expect(estimated.reviewedAt == savedReview.reviewedAt && estimated.selectedCardID == reviewed.selectedCardID)
    #expect(estimated.sourceSHA256 == reviewed.sourceSHA256 && estimated.finishEstimate != nil)
    let repeated = try await service.estimateFinish(jobID: record.id, sequence: 1, identificationID: request.id)
    #expect(repeated.finishEstimate == estimated.finishEstimate)
    await #expect(throws: ScannerError.self) { try await service.estimateFinish(jobID: record.id, sequence: 1, identificationID: UUID()) }

    let restarted = try ScannerCoordinator(backend: NoHardware(), storageRoot: root, database: database, cardRecognizer: stub)
    #expect(try await restarted.cardIdentification(jobID: record.id, sequence: 1)?.selectedCardID == "swsh3-136")
    let manifest = try Wire.decoder().decode(ScanRecord.self, from: Data(contentsOf: URL(fileURLWithPath: record.outputDirectory).appendingPathComponent("manifest.json")))
    #expect(manifest.cardIdentifications?.first?.finish == .reverseHolo)
    #expect(manifest.processedImages?.first?.id == edit.id)
    #expect(manifest.state == .completed && manifest.images.first?.sha256 == record.images.first?.sha256)
    _ = try await service.readImage(jobID: record.id, sequence: 1, offset: 0, length: 262144)
    let rejected = try await service.reviewCard(jobID: record.id, sequence: 1, identificationID: request.id, cardID: nil, finish: .normal, condition: .nearMint)
    #expect(rejected.state == .rejected && rejected.selectedCardID == nil && rejected.condition == .unassessed)
    await service.shutdownScans()
}

@Test func identificationFailuresAndRestartNeverAlterCompletedScan() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let database = try ScannerDatabase()
    var record = try savedBatch(root: root, database: database)
    let stub = RecognitionStub(fail: true)
    let service = try ScannerCoordinator(backend: NoHardware(), storageRoot: root, database: database, cardRecognizer: stub)
    _ = try await service.identifyCard(IdentificationRequest(jobID: record.id, sequence: 1))
    #expect(try await completed(service, job: record.id).state == .failed)
    #expect(try await service.scanStatus(record.id).state == .completed)
    record.cardIdentifications = [CardIdentification(request: IdentificationRequest(jobID: record.id, sequence: 1), sourceSHA256: record.images[0].sha256)]
    try database.save(record) // Simulate process death after accepting identification.
    #expect(try database.recoverInterruptedJobs(storageRoot: root) == 1)
    #expect(try database.job(record.id)?.cardIdentifications?.first?.state == .failed)
    #expect(try database.job(record.id)?.state == .completed)
    #expect(try database.recoverInterruptedJobs(storageRoot: root) == 0)
    let pending = try await service.identifyCard(IdentificationRequest(jobID: record.id, sequence: 1))
    await service.shutdownScans()
    let interrupted = try await service.cardIdentification(jobID: record.id, sequence: 1)
    #expect(interrupted?.request.id == pending.request.id && interrupted?.state == .failed)
}

@Test func recognitionRejectsUnsafePathsChangedOriginalsAndExtraRPCFields() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let database = try ScannerDatabase()
    let record = try savedBatch(root: root, database: database)
    let stub = RecognitionStub()
    let service = try ScannerCoordinator(backend: NoHardware(), storageRoot: root, database: database, cardRecognizer: stub)
    let file = URL(fileURLWithPath: record.outputDirectory).appendingPathComponent("C-001.png")
    var data = try Data(contentsOf: file); data[data.startIndex] ^= 1; try data.write(to: file)
    _ = try await service.identifyCard(IdentificationRequest(jobID: record.id, sequence: 1))
    #expect(try await completed(service, job: record.id).state == .failed)
    #expect(await stub.calls == 0)
    let router = DiagnosticRouter(coordinator: service)
    var request = RPCRequest(method: "v1.images.identify")
    request.params = .object(["jobID": .string(record.id.uuidString), "sequence": .number(1), "id": .string(UUID().uuidString), "path": .string("/etc/passwd")])
    #expect(await router.respond(to: try Wire.encoder().encode(request)).error?.code == -32602)
    request.params = .object(["jobID": .string(record.id.uuidString), "sequence": .number(1), "id": .string(UUID().uuidString), "language": .string("ja")])
    #expect(await router.respond(to: try Wire.encoder().encode(request)).error?.data?.kind == "invalidParams")
    await service.shutdownScans()
}
