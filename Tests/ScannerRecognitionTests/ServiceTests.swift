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
    func recognize(_ data: Data, language: CardLanguage) async throws -> RecognitionEvidence {
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
    var record = ScanRecord(device: Device(id: "fixture", name: "Saved image"), request: ScanRequest(duplex: true), outputDirectory: batch.path)
    record.state = .completed
    record.images = [OriginalImage(sequence: 1, relativePath: "C-001.png", bytes: data.count,
        sha256: SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined(), width: 120, height: 80, imageCount: 1, uniformType: "public.png")]
    var back = record.images[0]; back.sequence = 2; back.relativePath = "C-002.png"
    try data.write(to: batch.appendingPathComponent(back.relativePath))
    record.images.append(back)
    addPokemonBack(to: &record, sequence: 2)
    try data.write(to: batch.appendingPathComponent(try #require(record.processedImages?.last).image.relativePath))
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
    #expect(result.conditionCheck?.state == .unsupported)
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

    let checked = try await service.checkCondition(jobID: record.id, sequence: 1, identificationID: request.id)
    #expect(checked.conditionCheck?.state == .unsupported && checked.condition == .lightlyPlayed && checked.finish == .reverseHolo)
    #expect(checked.reviewedAt == savedReview.reviewedAt && checked.sourceSHA256 == reviewed.sourceSHA256)
    #expect(checked.finishEstimate == estimated.finishEstimate)
    let repeatedCheck = try await service.checkCondition(jobID: record.id, sequence: 1, identificationID: request.id)
    #expect(repeatedCheck.conditionCheck == checked.conditionCheck)
    await #expect(throws: ScannerError.self) { try await service.checkCondition(jobID: record.id, sequence: 1, identificationID: UUID()) }
    let restarted = try ScannerCoordinator(backend: NoHardware(), storageRoot: root, database: database, cardRecognizer: stub)
    #expect(try await restarted.cardIdentification(jobID: record.id, sequence: 1)?.selectedCardID == "swsh3-136")
    #expect(try await restarted.cardIdentification(jobID: record.id, sequence: 1)?.conditionCheck == checked.conditionCheck)
    let manifest = try Wire.decoder().decode(ScanRecord.self, from: Data(contentsOf: URL(fileURLWithPath: record.outputDirectory).appendingPathComponent("manifest.json")))
    #expect(manifest.cardIdentifications?.first?.finish == .reverseHolo)
    #expect(manifest.processedImages?.last?.id == edit.id)
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
    let failed = try await completed(service, job: record.id)
    #expect(failed.state == .failed && failed.conditionCheck?.state == .unsupported)
    let condition = try await service.checkCondition(jobID: record.id, sequence: 1, identificationID: failed.request.id)
    #expect(condition.state == .failed && condition.conditionCheck?.state == .unsupported)
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


@Test func conditionChecksAllowNoMatchBacksAndPreserveIdentity() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let database = try ScannerDatabase()
    var record = try savedBatch(root: root, database: database)
    var identity = CardIdentification(request: IdentificationRequest(jobID: record.id, sequence: 1), sourceSHA256: record.images[0].sha256)
    identity.state = .noMatch; identity.evidence = RecognitionEvidence(lines: [], candidates: [])
    record.cardIdentifications = [identity]; try database.save(record)
    let service = try ScannerCoordinator(backend: NoHardware(), storageRoot: root, database: database)
    let checked = try await service.checkCondition(jobID: record.id, sequence: 1, identificationID: identity.request.id)
    #expect(checked.state == .noMatch && checked.selectedCardID == nil && checked.condition == .unassessed)
    #expect(checked.conditionCheck?.state == .unsupported)
    var request = RPCRequest(method: "v1.images.checkCondition")
    request.params = .object(["jobID": .string(record.id.uuidString), "sequence": .number(1), "identificationID": .string(identity.request.id.uuidString), "path": .string("/etc/passwd")])
    let router = DiagnosticRouter(coordinator: service)
    #expect(await router.respond(to: try Wire.encoder().encode(request)).error?.code == -32602)
    await service.shutdownScans()
}

@Test(arguments: ["yugioh-back-pattern", "unknown"])
func otherGamesNeverInvokePokemonAnalysis(basis: String) async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let database = try ScannerDatabase()
    var record = try savedBatch(root: root, database: database)
    record.processedImages?[0].cardPreparation?.orientationBasis = basis
    try database.save(record)
    let stub = RecognitionStub(), yugioh = RecognitionStub()
    let service = try ScannerCoordinator(backend: NoHardware(), storageRoot: root, database: database, cardRecognizer: stub, yugiohRecognizer: yugioh)
    let request = IdentificationRequest(jobID: record.id, sequence: 1)
    _ = try await service.identifyCard(request)
    let result = try await completed(service, job: record.id)
    #expect(result.state == (basis == "yugioh-back-pattern" ? .reviewRequired : .noMatch))
    #expect(await yugioh.calls == (basis == "yugioh-back-pattern" ? 1 : 0))
    if basis == "yugioh-back-pattern" {
        #expect(result.method == "apple-vision-ygoprodeck-text-v1")
        _ = try await service.identifyCard(IdentificationRequest(jobID: record.id, sequence: 2))
        let deadline = ContinuousClock.now + .seconds(5)
        while try await service.cardIdentification(jobID: record.id, sequence: 2)?.state == .processing, ContinuousClock.now < deadline {
            try await Task.sleep(for: .milliseconds(10))
        }
        #expect(try await service.cardIdentification(jobID: record.id, sequence: 2)?.state == .noMatch)
        #expect(await yugioh.calls == 1) // Recognized back must never reach the front matcher.
    }
    #expect(result.finishEstimate == nil && result.conditionCheck == nil)
    #expect(await stub.calls == 0)
    await #expect(throws: ScannerError.self) { try await service.estimateFinish(jobID: record.id, sequence: 1, identificationID: request.id) }
    await #expect(throws: ScannerError.self) { try await service.checkCondition(jobID: record.id, sequence: 1, identificationID: request.id) }
    await service.shutdownScans()
}
