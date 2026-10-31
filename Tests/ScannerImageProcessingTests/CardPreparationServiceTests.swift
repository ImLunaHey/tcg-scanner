import CoreGraphics
import Foundation
import ImageIO
import Testing
import ScannerAPI
import ScannerCore
import ScannerPersistence
import ScannerRecognition

private actor AutomaticRecognition: CardRecognizing {
    var calls = 0
    let fail: Bool
    let delay: Duration
    init(fail: Bool = false, delay: Duration = .zero) { self.fail = fail; self.delay = delay }
    func recognize(_ data: Data) async throws -> RecognitionEvidence {
        calls += 1
        try await Task.sleep(for: delay)
        if fail { throw ScannerError("catalogueUnavailable", "Offline test") }
        return RecognitionEvidence(lines: [], candidates: [CardCandidate(id: "fixture-1", name: "Fixture", localID: "1", setID: "fixture", setName: "Fixture set", evidence: ["Test stub"])])
    }
}

private actor CardPaper: ScannerBackend {
    let blank: Bool
    let count: Int
    let tilt: Double
    init(blank: Bool = false, count: Int = 1, tilt: Double = 0) { self.blank = blank; self.count = count; self.tilt = tilt }
    func devices() -> [Device] { [Device(id: "mock", name: "Mock card source")] }
    func inspect(deviceID: String) -> Inspection { Inspection(device: devices()[0]) }
    func capture(deviceID: String, request: ScanRequest, store: ScanStorage) async throws -> CaptureOutcome {
        try await store.markScanning(settings: [:])
        let context = try #require(CGContext(data: nil, width: 400, height: 600, bitsPerComponent: 8, bytesPerRow: 1600,
            space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.setFillColor(red: 0, green: 0, blue: 0, alpha: 1); context.fill(CGRect(x: 0, y: 0, width: 400, height: 600))
        context.translateBy(x: 200, y: 300); context.rotate(by: tilt * .pi / 180); context.translateBy(x: -200, y: -300)
        if !blank { context.setFillColor(red: 0.8, green: 0.7, blue: 0.5, alpha: 1); context.fill(CGRect(x: 100, y: 250, width: 180, height: 250)) }
        let file = store.incomingDirectory.appendingPathComponent("mock.png")
        let destination = try #require(CGImageDestinationCreateWithURL(file as CFURL, "public.png" as CFString, 1, nil))
        CGImageDestinationAddImage(destination, try #require(context.makeImage()), nil)
        #expect(CGImageDestinationFinalize(destination))
        for sequence in 1...count { try await store.preserve(file, sequence: sequence) }
        return CaptureOutcome()
    }
}
private func waitForCard(_ coordinator: ScannerCoordinator, id: UUID) async throws -> ScanRecord {
    let deadline = ContinuousClock.now + .seconds(10)
    while ContinuousClock.now < deadline {
        let record = try await coordinator.scanStatus(id)
        if record.state.terminal { return record }
        try await Task.sleep(for: .milliseconds(10))
    }
    throw ScannerError("test", "Card processing timeout.")
}
@Test func optionalAutomaticCardPreparationPreservesOriginalAndManifest() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let database = try ScannerDatabase()
    let coordinator = try ScannerCoordinator(backend: CardPaper(), storageRoot: root, database: database, cardRecognizer: AutomaticRecognition())
    let start = try await coordinator.startScan(ScanRequest(prepareCards: true))
    let record = try await waitForCard(coordinator, id: start.id)
    #expect(record.state == .completed && record.images.count == 1)
    let prepared = try #require(record.processedImages?.first)
    let identity = try #require(record.cardIdentifications?.first)
    #expect(identity.state == .reviewRequired && identity.selectedCardID == nil)
    #expect(identity.sourceEditID == prepared.id && identity.sourceSHA256 == prepared.image.sha256)
    #expect(identity.finish == .unverified && identity.condition == .unassessed)
    #expect(identity.conditionCheck != nil) // Published automatically, without a client check request.
    #expect(prepared.cardPreparation != nil && prepared.image.width < 220 && prepared.image.height < 290)
    let original = try Data(contentsOf: URL(fileURLWithPath: record.outputDirectory).appendingPathComponent("C-001.png"))
    let id = UUID()
    let first = try await coordinator.prepareCard(jobID: record.id, sequence: 1, id: id)
    let repeated = try await coordinator.prepareCard(jobID: record.id, sequence: 1, id: id)
    #expect(first.image.sha256 == repeated.image.sha256)
    #expect(try await coordinator.scanStatus(record.id).processedImages?.count == 2)
    try database.save(record) // Simulate interruption between copy publication and ledger commit.
    let recovered = try await coordinator.prepareCard(jobID: record.id, sequence: 1, id: id)
    #expect(recovered.image.sha256 == first.image.sha256)
    #expect(try Data(contentsOf: URL(fileURLWithPath: record.outputDirectory).appendingPathComponent("C-001.png")) == original)
    let manifest = try Wire.decoder().decode(ScanRecord.self, from: Data(contentsOf: URL(fileURLWithPath: record.outputDirectory).appendingPathComponent("manifest.json")))
    #expect(manifest.processedImages?.count == 2)
    await coordinator.shutdownScans()
}
@Test func failedCardDetectionDoesNotLoseOrFailOriginalScan() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let coordinator = try ScannerCoordinator(backend: CardPaper(blank: true), storageRoot: root, cardRecognizer: AutomaticRecognition())
    let job = try await coordinator.startScan(ScanRequest(prepareCards: true))
    let record = try await waitForCard(coordinator, id: job.id)
    #expect(record.state == .completed && record.images.count == 1)
    #expect(record.processedImages == nil)
    #expect(record.cardIdentifications?.first?.sourceSHA256 == record.images.first?.sha256)
    #expect(record.cardIdentifications?.first?.state == .reviewRequired)
    #expect(record.warnings.contains { $0.contains("Card preparation failed") })
    await coordinator.shutdownScans()
}

@Test(arguments: [false, true]) func automaticIdentificationIsOptionalAndFailurePreservesScan(enabled: Bool) async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let recognizer = AutomaticRecognition(fail: true)
    let coordinator = try ScannerCoordinator(backend: CardPaper(count: 3), storageRoot: root, cardRecognizer: recognizer)
    let job = try await coordinator.startScan(ScanRequest(prepareCards: enabled))
    let result = try await waitForCard(coordinator, id: job.id)
    #expect(result.state == .completed && result.images.count == 3)
    #expect(await recognizer.calls == (enabled ? 3 : 0))
    if enabled {
        #expect(result.cardIdentifications?.map(\.request.sequence) == [1, 2, 3])
        #expect(result.cardIdentifications?.allSatisfy { $0.state == .failed } == true)
        #expect(result.cardIdentifications?.first?.state == .failed)
        #expect(result.cardIdentifications?.first?.error != nil)
    } else { #expect(result.cardIdentifications == nil) }
    let manifest = try Wire.decoder().decode(ScanRecord.self, from: Data(contentsOf: URL(fileURLWithPath: result.outputDirectory).appendingPathComponent("manifest.json")))
    #expect(manifest.cardIdentifications?.first?.state == result.cardIdentifications?.first?.state)
    await coordinator.shutdownScans()
}

@Test func automaticIdentificationPublishesProgressAndCancelsWithoutLosingImages() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let recognizer = AutomaticRecognition(delay: .seconds(30))
    let coordinator = try ScannerCoordinator(backend: CardPaper(), storageRoot: root, cardRecognizer: recognizer)
    let job = try await coordinator.startScan(ScanRequest(prepareCards: true))
    let deadline = ContinuousClock.now + .seconds(10)
    var pending: CardIdentification?
    while ContinuousClock.now < deadline {
        pending = try await coordinator.scanStatus(job.id).cardIdentifications?.first
        if pending != nil { break }
        try await Task.sleep(for: .milliseconds(10))
    }
    let identification = try #require(pending)
    #expect(identification.state == .processing)
    #expect(try await coordinator.scanStatus(job.id).state == .processing)
    await #expect(throws: ScannerError.self) {
        try await coordinator.identifyCard(IdentificationRequest(jobID: job.id, sequence: 1))
    }
    await #expect(throws: ScannerError.self) {
        try await coordinator.reviewCard(jobID: job.id, sequence: 1, identificationID: identification.request.id, cardID: nil, finish: .unverified, condition: .unassessed)
    }
    _ = try await coordinator.cancelScan(job.id)
    let result = try await waitForCard(coordinator, id: job.id)
    #expect(result.state == .cancelled && result.images.count == 1 && result.processedImages?.count == 1)
    #expect(result.cardIdentifications?.first?.state == .failed)
    #expect(result.cardIdentifications?.first?.error?.kind == "interrupted")
    await coordinator.shutdownScans()
}

@Test func automaticDeskewPersistsAndRendersItsTransform() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let coordinator = try ScannerCoordinator(backend: CardPaper(tilt: 5), storageRoot: root, cardRecognizer: AutomaticRecognition())
    let job = try await coordinator.startScan(ScanRequest(prepareCards: true))
    let record = try await waitForCard(coordinator, id: job.id)
    let copy = try #require(record.processedImages?.first)
    #expect(abs((copy.request.deskewDegrees ?? 0) - 5) < 0.2)
    #expect(copy.cardPreparation?.deskewDegrees == copy.request.deskewDegrees)
    #expect((180...187).contains(copy.image.width) && (250...257).contains(copy.image.height))
    #expect(record.images[0].width == 400 && record.images[0].height == 600)
    #expect(record.cardIdentifications?.first?.sourceSHA256 == copy.image.sha256)
    await coordinator.shutdownScans()
}

private actor SavedConditionPaper: ScannerBackend {
    let bytes: Data
    init(bytes: Data) { self.bytes = bytes }
    func devices() -> [Device] { [Device(id: "mock", name: "Saved back fixture")] }
    func inspect(deviceID: String) -> Inspection { Inspection(device: devices()[0]) }
    func capture(deviceID: String, request: ScanRequest, store: ScanStorage) async throws -> CaptureOutcome {
        try await store.markScanning(settings: [:])
        let url = store.incomingDirectory.appendingPathComponent("back.png")
        try bytes.write(to: url); try await store.preserve(url, sequence: 1)
        return CaptureOutcome()
    }
}
@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_CONDITION_FIXTURE"] != nil), arguments: [false, true])
func conditionChecksRunAutomaticallyWithoutClientRequest(catalogueFails: Bool) async throws {
    let path = try #require(ProcessInfo.processInfo.environment["SCANNER_CONDITION_FIXTURE"])
    let saved = try Wire.decoder().decode(ScanRecord.self, from: Data(contentsOf: URL(fileURLWithPath: path)))
    let source = try #require(saved.images.first { $0.sequence == 1 })
    let bytes = try Data(contentsOf: URL(fileURLWithPath: saved.outputDirectory).appendingPathComponent(source.relativePath))
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let coordinator = try ScannerCoordinator(backend: SavedConditionPaper(bytes: bytes), storageRoot: root,
        cardRecognizer: AutomaticRecognition(fail: catalogueFails))
    let job = try await coordinator.startScan(ScanRequest(prepareCards: true))
    let done = try await waitForCard(coordinator, id: job.id)
    let identity = try #require(done.cardIdentifications?.first)
    #expect(done.state == .completed)
    #expect(identity.conditionCheck?.state == .checked)
    #expect(identity.condition == .unassessed)
    #expect(identity.state == (catalogueFails ? .failed : .reviewRequired))
    let manifest = try Wire.decoder().decode(ScanRecord.self, from: Data(contentsOf: URL(fileURLWithPath: done.outputDirectory).appendingPathComponent("manifest.json")))
    #expect(manifest.cardIdentifications?.first?.conditionCheck == identity.conditionCheck)
    await coordinator.shutdownScans()
}
