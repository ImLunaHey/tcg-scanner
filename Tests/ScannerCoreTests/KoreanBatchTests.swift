import Foundation
import Testing
import ScannerAPI
import ScannerPersistence
import ScannerRecognition
@testable import ScannerCore

private actor KoreanNoHardware: ScannerBackend {
    func devices() throws -> [Device] { throw ScannerError("test", "No hardware calls in language tests.") }
    func inspect(deviceID: String) throws -> Inspection { throw ScannerError("test", "No hardware calls in language tests.") }
}
private actor KoreanNoPrices: CardPricingProviding {
    private(set) var calls = 0
    func prices(cardID: String) throws -> CardMarketPrices {
        calls += 1; throw ScannerError("test", "English pricing must not be requested for Korean cards.")
    }
    func sterlingRate() throws -> EuroSterlingRate {
        calls += 1; throw ScannerError("test", "No exchange request is needed without a supported price.")
    }
}
private struct KoreanNoStore: StoreStaging {
    let origin = "https://store.example"
    let scannerInstanceID = "language-test"
    func importBatch(_ value: StoreImport) throws -> StoreImportReply { throw ScannerError("test", "No Store writes.") }
    func update(card: StoreCardReply, input: StoreCardInput) throws -> StoreCardReply { throw ScannerError("test", "No Store writes.") }
    func upload(card: StoreCardReply, front: Data, back: Data) throws -> StoreCardReply { throw ScannerError("test", "No Store writes.") }
}

private func koreanRecord() -> ScanRecord {
    var record = ScanRecord(device: Device(id: "mock", name: "Mock"),
        request: ScanRequest(duplex: true, prepareCards: true, cardLanguage: .korean), outputDirectory: "/unused-language-tests/B-028")
    record.state = .completed; record.batchNumber = 28
    record.processedImages = []; record.cardIdentifications = []
    for sequence in 1...2 {
        let image = OriginalImage(sequence: sequence, relativePath: "\(sequence).png", bytes: 100,
            sha256: String(repeating: "a", count: 64), width: 200, height: 280, imageCount: 1, uniformType: "public.png")
        record.images.append(image)
        var prepared = ProcessedImage(request: ImageEditRequest(jobID: record.id, sequence: sequence), sourceSHA256: image.sha256, image: image)
        prepared.cardPreparation = CardPreparation(crop: ImageCrop(x: 0, y: 0, width: 1, height: 1), quarterTurns: 0,
            orientationVerified: true, warnings: [], orientationBasis: sequence == 1 ? "pokemon-back-pattern" : "text")
        record.processedImages?.append(prepared)
        var identity = CardIdentification(request: IdentificationRequest(jobID: record.id, sequence: sequence, language: "ko"), sourceSHA256: image.sha256)
        identity.sourceEditID = prepared.id; identity.state = sequence == 1 ? .noMatch : .reviewRequired
        identity.evidence = RecognitionEvidence(lines: [], candidates: sequence == 1 ? [] : [
            CardCandidate(id: "SV4M-001", name: "비구술", localID: "001", setID: "SV4M", setName: "미래의 일섬",
                officialSetCount: 66, evidence: ["Collector number 001"], availableFinishes: [.normal])])
        record.cardIdentifications?.append(identity)
    }
    return record
}

@Test func explicitKoreanLanguageRoundTripsAndRejectsUnsupportedInputs() throws {
    let request = ScanRequest(prepareCards: true, cardLanguage: .korean)
    #expect(try Wire.decoder().decode(ScanRequest.self, from: Wire.encoder().encode(request)) == request)
    #expect(request.resolvedCardLanguage == .korean && ScanRequest().resolvedCardLanguage == .english)
    try IdentificationRequest(jobID: UUID(), sequence: 1, language: "ko").validate()
    #expect(throws: ScannerError.self) { try IdentificationRequest(jobID: UUID(), sequence: 1, language: "Korean").validate() }
    #expect(throws: ScannerError.self) { try IdentificationRequest(jobID: UUID(), sequence: 1, language: "ja").validate() }
    var encoded = try #require(try JSONSerialization.jsonObject(with: Wire.encoder().encode(request)) as? [String: Any])
    encoded["cardLanguage"] = "ja"
    #expect(throws: (any Error).self) { try Wire.decoder().decode(ScanRequest.self, from: JSONSerialization.data(withJSONObject: encoded)) }
}

@Test func koreanFinishNeverUsesEnglishSetCodeBorderOrStampRules() async throws {
    let identity = try #require(koreanRecord().cardIdentifications?.last)
    let evidence = try #require(identity.evidence)
    // Empty image bytes would fail image analysis. Korean exits at its explicit
    // scope guard before any image/English-catalogue call and reports no guess.
    let result = await CardFinishAnalyzer().estimate(Data(), evidence: evidence, preparation: nil, language: .korean)
    #expect(result.state == .unsupported && result.uncertaintyReason == .languageUnsupported)
    #expect(result.candidateID == "SV4M-001" && result.suggestedFinish == nil)
    #expect(result.edition == nil && result.borderEvidence == nil)
}

@Test func koreanValuationDoesNotFetchOrSubstituteEnglishPrices() async throws {
    let db = try ScannerDatabase(), record = koreanRecord(), prices = KoreanNoPrices()
    try db.insert(record, submission: record.request)
    let coordinator = try ScannerCoordinator(backend: KoreanNoHardware(), database: db, cardPricing: prices)
    let value = try await coordinator.batchValuation(record.id)
    #expect(value.cards.count == 1 && value.cards[0].state == .unavailable)
    #expect(value.cards[0].cardID == "SV4M-001" && value.cards[0].reason.contains("Korean"))
    #expect(value.pricedCount == 0 && value.lowPence == nil && !value.refreshing && value.exchangeRate == nil)
    #expect(await prices.calls == 0)
}

@Test func koreanStoreMetadataPreservesPhysicalLanguageAndRejectsContradictions() throws {
    var record = koreanRecord()
    // Even a stale/injected English estimate or quote cannot become Korean evidence.
    record.cardIdentifications?[1].finishEstimate = FinishEstimate(state: .estimated, suggestedFinish: .holo,
        candidateID: "SV4M-001", evidence: ["English fixture estimate"])
    var quote = CardValuation(number: 1, reason: "English quote")
    quote.cardID = "SV4M-001"; quote.state = .priced; quote.lowPence = 99; quote.highPence = 99
    let valuation = BatchValuation(batchID: record.id, cards: [quote], exchangeRate: nil, refreshing: false)
    let plan = try StoreTransferLedger.make(record: record, store: KoreanNoStore(), valuation: valuation)
    let card = try #require(plan.submission.cards.first)
    #expect(card.language == "ko" && card.languageSource == "scannerConfiguration")
    #expect(card.cardNumber == "C-001" && plan.submission.batchNumber == "B-028")
    #expect(card.review.recognitionSuggestions.first?.language == "ko")
    #expect(card.review.recognitionSuggestions.first?.name == "비구술")
    #expect(card.review.suggestedPriceGbp == nil && card.review.finishEstimate == nil)
    record.cardIdentifications?[1].state = .noMatch
    record.cardIdentifications?[1].evidence?.candidates = []
    let unknown = try StoreTransferLedger.make(record: record, store: KoreanNoStore(), valuation: nil).submission.cards[0]
    #expect(unknown.language == "ko" && unknown.review.recognitionSuggestions.isEmpty)
    record.cardIdentifications?[1].request.language = "en"
    #expect(throws: ScannerError.self) { try StoreTransferLedger.make(record: record, store: KoreanNoStore(), valuation: nil) }
}

@Test func koreanBatchRejectsEnglishIdentificationRetryBeforeProcessing() async throws {
    let db = try ScannerDatabase(), record = koreanRecord()
    try db.insert(record, submission: record.request)
    let coordinator = try ScannerCoordinator(backend: KoreanNoHardware(), database: db)
    do {
        _ = try await coordinator.identifyCard(IdentificationRequest(jobID: record.id, sequence: 2, language: "en"))
        Issue.record("Mismatched physical card language must be rejected")
    } catch let error as ScannerError { #expect(error.kind == "invalidParams" && error.message.contains("ko")) }
    #expect(try Wire.encoder().encode(db.job(record.id)) == Wire.encoder().encode(record))
}
