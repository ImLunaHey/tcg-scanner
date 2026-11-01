import Foundation
import Testing
import ScannerAPI
import ScannerPersistence
import ScannerRecognition
@testable import ScannerCore

private actor StampGuardHardware: ScannerBackend {
    func devices() throws -> [Device] { throw ScannerError("test", "No hardware calls during export checks.") }
    func inspect(deviceID: String) throws -> Inspection { throw ScannerError("test", "No hardware calls during export checks.") }
}

private struct StampGuardPrices: CardPricingProviding {
    func prices(cardID: String) async throws -> CardMarketPrices { throw ScannerError("test", "No live pricing.") }
    func sterlingRate() async throws -> EuroSterlingRate { throw ScannerError("test", "No live exchange rates.") }
}

private actor StampGuardStore: StoreStaging {
    let origin = "https://store.example"
    let scannerInstanceID = "edition-guard-fixture"
    private(set) var importCalls = 0
    private(set) var updateCalls = 0
    private(set) var uploadCalls = 0
    private let duringImport: (@Sendable () throws -> Void)?
    init(duringImport: (@Sendable () throws -> Void)? = nil) { self.duringImport = duringImport }
    func importBatch(_ value: StoreImport) throws -> StoreImportReply {
        importCalls += 1
        try duringImport?()
        let batch = UUID()
        return StoreImportReply(batch: StoreBatchReply(id: batch, scannerInstanceId: value.scannerInstanceId,
            sourceBatchId: value.sourceBatchId, batchNumber: value.batchNumber, reviewUrl: "/admin/scan-inbox/\(batch)"),
            cards: value.cards.map { StoreCardReply(id: UUID(), scanBatchId: batch, cardNumber: $0.cardNumber,
                revision: 1, status: "pending", front: nil, back: nil) })
    }
    func update(card: StoreCardReply, input: StoreCardInput) throws -> StoreCardReply {
        updateCalls += 1
        throw ScannerError("test", "Edition guard must stop before metadata writes.")
    }
    func upload(card: StoreCardReply, front: Data, back: Data) throws -> StoreCardReply {
        uploadCalls += 1
        throw ScannerError("test", "Edition guard must stop before image uploads.")
    }
}

private func stampGuardRecord() -> ScanRecord {
    var record = ScanRecord(device: Device(id: "fixture", name: "Fixture"),
        request: ScanRequest(duplex: true, prepareCards: true), outputDirectory: "/unused-store-guard/B-027")
    record.state = .completed; record.batchNumber = 27
    record.processedImages = []; record.cardIdentifications = []
    for sequence in 1...2 {
        let image = OriginalImage(sequence: sequence, relativePath: "\(sequence).png", bytes: 1024,
            sha256: String(repeating: "a", count: 64), width: 200, height: 280, imageCount: 1, uniformType: "public.png")
        record.images.append(image)
        var prepared = ProcessedImage(request: ImageEditRequest(jobID: record.id, sequence: sequence),
            sourceSHA256: image.sha256, image: image)
        prepared.cardPreparation = CardPreparation(crop: ImageCrop(x: 0, y: 0, width: 1, height: 1), quarterTurns: 0,
            orientationVerified: true, warnings: [], orientationBasis: sequence == 1 ? "pokemon-back-pattern" : "text")
        record.processedImages?.append(prepared)
        var identity = CardIdentification(request: IdentificationRequest(jobID: record.id, sequence: sequence), sourceSHA256: image.sha256)
        identity.sourceEditID = prepared.id; identity.state = sequence == 1 ? .noMatch : .reviewRequired
        identity.evidence = RecognitionEvidence(lines: [], candidates: sequence == 1 ? [] : [
            CardCandidate(id: "sv02-097", name: "Mimikyu", localID: "097", setID: "sv02", setName: "Paldea Evolved", evidence: ["Collector number 097"])
        ])
        record.cardIdentifications?.append(identity)
    }
    return record
}

private func addingStamp(_ record: ScanRecord, state: CardStampEvidence.State = .present) -> ScanRecord {
    var result = record
    var estimate = FinishEstimate(state: .estimated, suggestedFinish: .holo, candidateID: "sv02-097", evidence: [])
    estimate.edition = CardEditionEvidence(state: state == .absent ? .notDetected : state == .uncertain ? .uncertain : .matched, candidateID: "sv02-097", stamp: CardStampEvidence(state: state),
        name: "Trick or Trade", variants: [CardPrintingVariant(id: "stamped", finish: .holo, stamps: ["trick-or-trade"])])
    result.cardIdentifications?[1].finishEstimate = estimate
    return result
}

@Test func stampedStorePlansRejectDetectedAndUncertainMarksEvenAfterReview() throws {
    let store = StampGuardStore()
    for state: CardStampEvidence.State in [.present, .uncertain] {
        var record = addingStamp(stampGuardRecord(), state: state)
        record.cardIdentifications?[1].state = .confirmed
        record.cardIdentifications?[1].selectedCardID = "sv02-097"
        record.cardIdentifications?[1].finish = .holo
        // A mismatched catalogue selection cannot make physical stamp evidence disappear.
        record.cardIdentifications?[1].finishEstimate?.edition?.candidateID = "different-base-id"
        do {
            _ = try StoreTransferLedger.make(record: record, store: store, valuation: nil)
            Issue.record("Expected the unsupported Store edition contract to block this card")
        } catch let error as ScannerError {
            #expect(error.kind == "storeNotReady")
            #expect(error.message.contains("C-001") && error.message.contains("stamped-edition metadata"))
        }
    }
    let ordinary = try StoreTransferLedger.make(record: stampGuardRecord(), store: store, valuation: nil)
    #expect(ordinary.cards.count == 1)
    let absent = try StoreTransferLedger.make(record: addingStamp(stampGuardRecord(), state: .absent), store: store, valuation: nil)
    #expect(absent.cards.count == 1)
}

@Test func failedStoreRetryChecksNewStampEvidenceBeforeReusingOldSubmission() async throws {
    let db = try ScannerDatabase(), store = StampGuardStore(), original = stampGuardRecord()
    try db.insert(original, submission: original.request)
    var ledger = try StoreTransferLedger.make(record: original, store: store, valuation: nil)
    ledger.status.state = .failed
    let frozen = try Wire.encoder().encode(ledger)
    try db.saveStoreTransfer(ledger.status, record: frozen)
    let stamped = addingStamp(original)
    try db.save(stamped)
    let coordinator = try ScannerCoordinator(backend: StampGuardHardware(), database: db,
        cardPricing: StampGuardPrices(), storeClient: store)
    do {
        _ = try await coordinator.sendToStore(original.id)
        Issue.record("A frozen ordinary-printing retry must not bypass current stamp evidence")
    } catch let error as ScannerError {
        #expect(error.kind == "storeNotReady" && error.message.contains("C-001"))
    }
    #expect(await store.importCalls == 0)
    #expect(await store.updateCalls == 0)
    #expect(await store.uploadCalls == 0)
    #expect(try db.storeTransfer(original.id) == frozen)
    #expect(try db.job(original.id)?.cardIdentifications?[1].finishEstimate?.edition?.stamp.state == .present)
}

@Test func activeStoreStatusRemainsIdempotentWithoutNewCallsForStampedBatch() async throws {
    let db = try ScannerDatabase(), store = StampGuardStore(), original = stampGuardRecord()
    try db.insert(original, submission: original.request)
    let coordinator = try ScannerCoordinator(backend: StampGuardHardware(), database: db,
        cardPricing: StampGuardPrices(), storeClient: store)
    var ledger = try StoreTransferLedger.make(record: original, store: store, valuation: nil)
    ledger.status.state = .sending
    try db.saveStoreTransfer(ledger.status, record: Wire.encoder().encode(ledger))
    try db.save(addingStamp(original))
    #expect(try await coordinator.sendToStore(original.id).state == .sending)
    #expect(await store.importCalls == 0)
    #expect(await store.updateCalls == 0)
    #expect(await store.uploadCalls == 0)
}

@Test func newlyDetectedStampStopsRemainingWritesAfterAnInFlightImport() async throws {
    let db = try ScannerDatabase(), original = stampGuardRecord()
    try db.insert(original, submission: original.request)
    let stamped = addingStamp(original)
    let store = StampGuardStore(duringImport: { try db.save(stamped) })
    let coordinator = try ScannerCoordinator(backend: StampGuardHardware(), database: db,
        cardPricing: StampGuardPrices(), storeClient: store)
    #expect(try await coordinator.sendToStore(original.id).state == .sending)
    var status = try await coordinator.storeTransferStatus(original.id)
    for _ in 0..<200 where status.state == .sending {
        try await Task.sleep(for: .milliseconds(5))
        status = try await coordinator.storeTransferStatus(original.id)
    }
    #expect(status.state == .failed && status.error?.kind == "storeNotReady")
    #expect(status.error?.message.contains("C-001") == true)
    #expect(await store.importCalls == 1)
    #expect(await store.updateCalls == 0)
    #expect(await store.uploadCalls == 0)
    await coordinator.shutdownScans()
}

@Test func failedIdentificationRetryCannotEraseTheSameSourceStamp() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    var record = addingStamp(stampGuardRecord())
    record.outputDirectory = root.appendingPathComponent("B-027").path
    try FileManager.default.createDirectory(atPath: record.outputDirectory, withIntermediateDirectories: true)
    record.cardIdentifications?[1].state = .rejected
    let db = try ScannerDatabase(), store = StampGuardStore()
    try db.insert(record, submission: record.request)
    let coordinator = try ScannerCoordinator(backend: StampGuardHardware(), storageRoot: root,
        database: db, cardPricing: StampGuardPrices(), storeClient: store)
    let request = IdentificationRequest(jobID: record.id, sequence: 2)
    let pending = try await coordinator.identifyCard(request)
    #expect(pending.finishEstimate?.edition?.stamp.state == .present)
    var result = try await coordinator.cardIdentification(jobID: record.id, sequence: 2)
    for _ in 0..<200 where result?.state == .processing {
        try await Task.sleep(for: .milliseconds(5))
        result = try await coordinator.cardIdentification(jobID: record.id, sequence: 2)
    }
    // The absent image deliberately fails the new attempt; its earlier mark must survive.
    #expect(result?.state == .failed && result?.finishEstimate?.edition?.stamp.state == .present)
    await #expect(throws: ScannerError.self) { try await coordinator.sendToStore(record.id) }
    #expect(await store.importCalls == 0)
    #expect(await store.uploadCalls == 0)
    await coordinator.shutdownScans()
}

@Test func storePriceMustMatchCurrentCardAndEdition() throws {
    let record = stampGuardRecord(), store = StampGuardStore()
    var quote = CardValuation(number: 1, reason: "fixture")
    quote.cardID = "sv02-097"; quote.state = .priced; quote.lowPence = 12; quote.highPence = 12
    func price(_ quote: CardValuation) throws -> Int? {
        let valuation = BatchValuation(batchID: record.id, cards: [quote], exchangeRate: nil, refreshing: false)
        return try StoreTransferLedger.make(record: record, store: store, valuation: valuation).submission.cards[0].review.suggestedPriceGbp
    }
    #expect(try price(quote) == 12)
    quote.cardID = "another-card"
    #expect(try price(quote) == nil)
    quote.cardID = "sv02-097"; quote.editionKey = "different-edition"
    #expect(try price(quote) == nil)
}
