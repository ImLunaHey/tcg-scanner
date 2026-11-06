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
    func recognize(_ data: Data, language: CardLanguage) async throws -> RecognitionEvidence {
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
    #expect(record.cardIdentifications == nil)
    #expect(record.cardGames?.first?.game == .unknown)
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
    #expect(record.cardIdentifications == nil)
    #expect(record.warnings.contains { $0.contains("Card preparation failed") })
    await coordinator.shutdownScans()
}

@Test(arguments: [false, true]) func unrecognizedPairsNeverInvokePokemonRecognizer(enabled: Bool) async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let recognizer = AutomaticRecognition(fail: true)
    let coordinator = try ScannerCoordinator(backend: CardPaper(count: 3), storageRoot: root, cardRecognizer: recognizer)
    let job = try await coordinator.startScan(ScanRequest(duplex: true, prepareCards: enabled))
    let result = try await waitForCard(coordinator, id: job.id)
    #expect(result.state == .completed && result.images.count == 3)
    #expect(await recognizer.calls == 0)
    #expect(result.cardIdentifications == nil)
    #expect(result.cardGames?.allSatisfy { $0.game == .unknown } == true)
    let manifest = try Wire.decoder().decode(ScanRecord.self, from: Data(contentsOf: URL(fileURLWithPath: result.outputDirectory).appendingPathComponent("manifest.json")))
    #expect(manifest.cardIdentifications?.first?.state == result.cardIdentifications?.first?.state)
    await coordinator.shutdownScans()
}

@Test func automaticPreparationPublishesProgressAndCancelsWithoutLosingImages() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let recognizer = AutomaticRecognition()
    let coordinator = try ScannerCoordinator(backend: CardPaper(count: 20), storageRoot: root, cardRecognizer: recognizer)
    let job = try await coordinator.startScan(ScanRequest(duplex: true, prepareCards: true))
    let deadline = ContinuousClock.now + .seconds(10)
    var current = try await coordinator.scanStatus(job.id)
    while current.processedImages?.isEmpty != false, ContinuousClock.now < deadline {
        try await Task.sleep(for: .milliseconds(10))
        current = try await coordinator.scanStatus(job.id)
    }
    #expect(current.state == .processing)
    #expect(current.processedImages?.isEmpty == false)
    _ = try await coordinator.cancelScan(job.id)
    let result = try await waitForCard(coordinator, id: job.id)
    #expect(result.state == .cancelled && result.images.count == 20)
    #expect(result.processedImages?.isEmpty == false)
    #expect(await recognizer.calls == 0)
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
    #expect(record.cardIdentifications == nil)
    await coordinator.shutdownScans()
}

private actor SavedConditionPaper: ScannerBackend {
    let bytes: [Data]
    init(bytes: [Data]) { self.bytes = bytes }
    func devices() -> [Device] { [Device(id: "mock", name: "Saved back fixture")] }
    func inspect(deviceID: String) -> Inspection { Inspection(device: devices()[0]) }
    func capture(deviceID: String, request: ScanRequest, store: ScanStorage) async throws -> CaptureOutcome {
        try await store.markScanning(settings: [:])
        let url = store.incomingDirectory.appendingPathComponent("back.png")
        for (index, data) in bytes.enumerated() {
            try data.write(to: url); try await store.preserve(url, sequence: index + 1)
        }
        return CaptureOutcome()
    }
}
@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_CONDITION_FIXTURE"] != nil), arguments: [false, true])
func conditionChecksRunAutomaticallyWithoutClientRequest(catalogueFails: Bool) async throws {
    let path = try #require(ProcessInfo.processInfo.environment["SCANNER_CONDITION_FIXTURE"])
    let saved = try Wire.decoder().decode(ScanRecord.self, from: Data(contentsOf: URL(fileURLWithPath: path)))
    for sourceSequence in [1, 11, 17] {
        let source = try #require(saved.images.first { $0.sequence == sourceSequence })
        let front = try #require(saved.images.first { $0.sequence == sourceSequence + 1 })
        let bytes = try [source, front].map { try Data(contentsOf: URL(fileURLWithPath: saved.outputDirectory).appendingPathComponent($0.relativePath)) }
        let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: root) }
        let coordinator = try ScannerCoordinator(backend: SavedConditionPaper(bytes: bytes), storageRoot: root,
            cardRecognizer: AutomaticRecognition(fail: catalogueFails))
        let job = try await coordinator.startScan(ScanRequest(duplex: true, prepareCards: true))
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
}

@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_GAME_FIXTURE"] != nil))
func savedPairsAutomaticallyPrepareAndRouteByGame() async throws {
    let path = try #require(ProcessInfo.processInfo.environment["SCANNER_GAME_FIXTURE"])
    let expected = CardGame(rawValue: ProcessInfo.processInfo.environment["SCANNER_GAME_EXPECTED"] ?? "yugioh")!
    let count = Int(ProcessInfo.processInfo.environment["SCANNER_GAME_COUNT"] ?? "8")!
    let bytes = try (1...count).map {
        try Data(contentsOf: URL(fileURLWithPath: path).appendingPathComponent(String(format: "C-%03d.png", $0)))
    }
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let recognizer = AutomaticRecognition()
    let coordinator = try ScannerCoordinator(backend: SavedConditionPaper(bytes: bytes), storageRoot: root, cardRecognizer: recognizer, yugiohRecognizer: recognizer)
    let job = try await coordinator.startScan(ScanRequest(duplex: true, prepareCards: true))
    let deadline = ContinuousClock.now + .seconds(120)
    var result = try await coordinator.scanStatus(job.id)
    while !result.state.terminal && ContinuousClock.now < deadline {
        try await Task.sleep(for: .milliseconds(100))
        result = try await coordinator.scanStatus(job.id)
    }
    #expect(result.state == .completed && result.images.count == count)
    #expect(result.processedImages?.count == count)
    #expect(result.cardGames?.count == count / 2)
    #expect(result.cardGames?.allSatisfy { $0.game == expected } == true)
    #expect(await recognizer.calls == (count / 2))
    if expected == .yugioh {
        #expect(result.cardIdentifications?.allSatisfy { $0.request.sequence.isMultiple(of: 2) && $0.finishEstimate == nil && $0.conditionCheck == nil } == true)
    }
    let manifest = try Wire.decoder().decode(ScanRecord.self, from: Data(contentsOf: URL(fileURLWithPath: result.outputDirectory).appendingPathComponent("manifest.json")))
    #expect(manifest.cardGames?.map(\.game) == result.cardGames?.map(\.game))
    await coordinator.shutdownScans()
}
