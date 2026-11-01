import Foundation
import Testing
import ScannerAPI
import ScannerCore
import ScannerPersistence
import ScannerServiceSupport
@testable import ScannerRecognition

private struct PricingNoHardware: ScannerBackend {
    func devices() throws -> [Device] { throw ScannerError("test", "Pricing must not access hardware") }
    func inspect(deviceID: String) throws -> Inspection { throw ScannerError("test", "Pricing must not access hardware") }
}
private actor PriceStub: CardPricingProviding {
    var calls = 0
    var fxCalls = 0
    var offline: Bool
    init(offline: Bool = false) { self.offline = offline }
    func prices(cardID: String) async throws -> CardMarketPrices {
        calls += 1
        try await Task.sleep(for: .milliseconds(20))
        if offline { throw ScannerError("offline", "Fixture") }
        return quote(cardID)
    }
    func sterlingRate() throws -> EuroSterlingRate {
        fxCalls += 1
        if offline { throw ScannerError("offline", "Fixture") }
        return EuroSterlingRate(poundsPerEuro: Decimal(string: "0.835")!, date: today())
    }
}
private func today() -> String { String(ISO8601DateFormatter().string(from: Date()).prefix(10)) }
private func quote(_ id: String = "test-001", age: TimeInterval = 0) -> CardMarketPrices {
    CardMarketPrices(cardID: id, fetchedAt: Date().addingTimeInterval(-age), variants: [
        MarketVariantPrice(finish: .normal, euros: Decimal(string: "0.03"), updated: ISO8601DateFormatter().string(from: Date())),
        MarketVariantPrice(finish: .holo, euros: Decimal(string: "1.25"), updated: ISO8601DateFormatter().string(from: Date())),
        MarketVariantPrice(finish: .reverseHolo, euros: Decimal(string: "0.25"), updated: ISO8601DateFormatter().string(from: Date()))])
}
private func batch(_ database: ScannerDatabase, finishes: [CardFinish?]) throws -> ScanRecord {
    var record = ScanRecord(device: Device(id: "mock", name: "Mock"), request: ScanRequest(duplex: true, prepareCards: true), outputDirectory: "/unused")
    record.state = .completed
    record.cardIdentifications = []
    for (index, finish) in finishes.enumerated() {
        for sequence in (index * 2 + 1)...(index * 2 + 2) {
            record.images.append(OriginalImage(sequence: sequence, relativePath: "\(sequence).png", bytes: 0, sha256: "fixture", width: 100, height: 100, imageCount: 1, uniformType: "public.png"))
        }
        // Alternating which capture is the card front must not change card totals.
        var identity = CardIdentification(request: IdentificationRequest(jobID: record.id, sequence: index * 2 + (index.isMultiple(of: 2) ? 1 : 2)), sourceSHA256: "fixture")
        identity.state = .reviewRequired
        identity.evidence = RecognitionEvidence(lines: [], candidates: [CardCandidate(id: "test-001", name: "Test", localID: "001", setID: "test", setName: "Test", evidence: [], availableFinishes: [.normal, .holo, .reverseHolo])])
        identity.finishEstimate = FinishEstimate(state: finish == nil ? .uncertain : .estimated, suggestedFinish: finish, candidateID: "test-001", evidence: [], uncertaintyReason: finish == nil ? .multipleRegularFinishes : nil)
        record.cardIdentifications?.append(identity)
    }
    try database.insert(record, submission: record.request)
    return record
}
private func ready(_ service: ScannerCoordinator, id: UUID) async throws -> BatchValuation {
    let deadline = ContinuousClock.now + .seconds(3)
    while ContinuousClock.now < deadline {
        let value = try await service.batchValuation(id)
        if !value.refreshing { return value }
        try await Task.sleep(for: .milliseconds(10))
    }
    throw ScannerError("test", "Pricing did not complete")
}

@Test func pricingUsesVariantProductsAndNeverSubstitutesMissingQuotes() throws {
    let data = Data(#"{"id":"test-1","variants":{"normal":true,"holo":true,"reverse":true},"variants_detailed":[{"type":"normal","size":"standard","pricing":{"cardmarket":{"unit":"EUR","idProduct":1,"trend":0.03,"trend-holo":0.25}}},{"type":"holo","size":"standard","pricing":{"cardmarket":{"unit":"EUR","idProduct":2,"trend":1.25,"trend-holo":9}}},{"type":"reverse","size":"standard","pricing":{"cardmarket":{"unit":"EUR","idProduct":1,"trend":0.03,"trend-holo":0}}},{"type":"holo","size":"standard","stamp":["prerelease"],"pricing":{"cardmarket":{"unit":"EUR","trend":500}}}] }"#.utf8)
    let prices = try JSONDecoder().decode(PricingDetail.self, from: data).quotes()
    #expect(prices.variants.count == 3)
    #expect(prices.variants[0].euros == Decimal(string: "0.03"))
    #expect(prices.variants[1].euros == Decimal(string: "1.25"))
    #expect(prices.variants[2].euros == nil)
    let wrongCurrency = Data(#"{"id":"test-1","variants":{"normal":true},"pricing":{"cardmarket":{"unit":"USD","trend":5}}}"#.utf8)
    #expect(try JSONDecoder().decode(PricingDetail.self, from: wrongCurrency).quotes().variants[0].euros == nil)
}

@Test func pricingRejectsAmbiguousRegularProductAndOldOrInvalidFX() throws {
    let data = Data(#"{"id":"test-1","variants_detailed":[{"type":"normal","size":"standard","pricing":{"cardmarket":{"unit":"EUR","idProduct":1,"trend":5}}},{"type":"holo","size":"standard","pricing":{"cardmarket":{"unit":"EUR","idProduct":1,"trend":5}}}]}"#.utf8)
    #expect(try JSONDecoder().decode(PricingDetail.self, from: data).quotes().variants.allSatisfy { $0.euros == nil })
    let valid = Data("<Cube><Cube time='\(today())'><Cube currency='GBP' rate='0.835'/></Cube></Cube>".utf8)
    #expect(try CardPricingProvider.decodeRate(valid).poundsPerEuro == Decimal(string: "0.835"))
    for xml in ["<Cube/>", "<Cube time='2000-01-01'><Cube currency='GBP' rate='0.8'/></Cube>", "<Cube time='\(today())'><Cube currency='GBP' rate='-2'/></Cube>"] {
        #expect(throws: ScannerError.self) { try CardPricingProvider.decodeRate(Data(xml.utf8)) }
    }
}

@Test func batchPricingIsAsyncCoalescedRoundedAndCountsPhysicalDuplicates() async throws {
    let db = try ScannerDatabase()
    let record = try batch(db, finishes: [.normal, .normal, nil, .reverseHolo])
    let stub = PriceStub()
    let service = try ScannerCoordinator(backend: PricingNoHardware(), database: db, cardPricing: stub)
    let first = try await service.batchValuation(record.id)
    #expect(first.refreshing && first.cards.allSatisfy { $0.state == .pending })
    async let a = ready(service, id: record.id)
    async let b = ready(service, id: record.id)
    let (result, other) = try await (a, b)
    #expect(result.pricedCount == 4 && result.totalCount == 4)
    #expect(result.cards.map(\.lowPence) == [3, 3, 3, 21])
    #expect(result.cards.map(\.highPence) == [3, 3, 104, 21])
    #expect(result.lowPence == 30 && result.highPence == 131)
    #expect(other.lowPence == result.lowPence)
    #expect(await stub.calls == 1)
    #expect(await stub.fxCalls == 1)
    #expect(try Wire.encoder().encode(db.job(record.id)) == Wire.encoder().encode(record))
    let restarted = try ScannerCoordinator(backend: PricingNoHardware(), database: db, cardPricing: stub)
    #expect(try await restarted.batchValuation(record.id).lowPence == 30)
    #expect(await stub.calls == 1)
}

@Test func retroactivelyPreparedDuplexCardsArePricedOncePerPair() async throws {
    let db = try ScannerDatabase()
    var record = try batch(db, finishes: [.normal, .normal])
    record.request.prepareCards = false
    var edit = ProcessedImage(request: ImageEditRequest(jobID: record.id, sequence: 1), sourceSHA256: "fixture", image: record.images[0])
    edit.cardPreparation = CardPreparation(crop: ImageCrop(x: 0, y: 0, width: 1, height: 1), quarterTurns: 0, orientationVerified: true, warnings: [])
    record.processedImages = [edit]
    try db.save(record)
    let service = try ScannerCoordinator(backend: PricingNoHardware(), database: db, cardPricing: PriceStub())
    let value = try await ready(service, id: record.id)
    #expect(value.totalCount == 2 && value.pricedCount == 2 && value.lowPence == 6)
}

@Test func missingFinishQuoteMakesCardUnavailableNotAnIncompleteRange() async throws {
    let db = try ScannerDatabase()
    let record = try batch(db, finishes: [nil, .normal])
    var prices = quote(); prices.variants.removeAll { $0.finish == .holo }
    try db.cachePrices(prices)
    try db.cacheSterlingRate(EuroSterlingRate(poundsPerEuro: Decimal(string: "0.835")!, date: today()))
    let service = try ScannerCoordinator(backend: PricingNoHardware(), database: db, cardPricing: PriceStub())
    let result = try await service.batchValuation(record.id)
    #expect(result.pricedCount == 1 && result.totalCount == 2 && result.lowPence == 3)
    #expect(result.cards[0].state == .unavailable && result.cards[0].lowPence == nil)
}

@Test func rejectedAmbiguousAndIncompletePairsAreNeverValued() async throws {
    let db = try ScannerDatabase()
    var record = try batch(db, finishes: [.normal, .normal, .normal])
    record.cardIdentifications?[0].state = .rejected
    var both = record.cardIdentifications![1]; both.request.sequence = 3
    record.cardIdentifications?.append(both)
    record.images.removeAll { $0.sequence == 6 }
    try db.save(record)
    let stub = PriceStub()
    let service = try ScannerCoordinator(backend: PricingNoHardware(), database: db, cardPricing: stub)
    let result = try await service.batchValuation(record.id)
    #expect(result.pricedCount == 0 && result.lowPence == nil && result.totalCount == 3)
    #expect(await stub.calls == 0)
    #expect(await stub.fxCalls == 0)
}

@Test func staleCacheSurvivesOfflineProviderAndLimitsRetries() async throws {
    let db = try ScannerDatabase()
    let record = try batch(db, finishes: [.normal])
    try db.cachePrices(quote(age: 7 * 3600))
    try db.cacheSterlingRate(EuroSterlingRate(poundsPerEuro: Decimal(string: "0.835")!, date: today(), fetchedAt: Date().addingTimeInterval(-7 * 3600)))
    let stub = PriceStub(offline: true)
    let service = try ScannerCoordinator(backend: PricingNoHardware(), database: db, cardPricing: stub)
    let result = try await ready(service, id: record.id)
    #expect(result.stale && result.lowPence == 3)
    for _ in 0..<10 { _ = try await service.batchValuation(record.id) }
    #expect(await stub.calls == 1)
    #expect(await stub.fxCalls == 1)
    try db.cachePrices(quote(age: 8 * 86400))
    #expect(try await service.batchValuation(record.id).pricedCount == 0)
}

@Test func pricingRPCValidatesParametersAndReturnsStableContract() async throws {
    let db = try ScannerDatabase()
    let record = try batch(db, finishes: [])
    let router = DiagnosticRouter(coordinator: try ScannerCoordinator(backend: PricingNoHardware(), database: db, cardPricing: PriceStub()), backendName: "mock")
    var request = RPCRequest(method: "v1.batches.valuation")
    request.params = .object(["id": .string(record.id.uuidString)])
    let response = await router.respond(to: try Wire.encoder().encode(request))
    #expect(response.error == nil)
    let result = try #require(response.result).decode(BatchValuation.self)
    #expect(result.currency == "GBP" && result.totalCount == 0 && result.lowPence == nil)
    request.params = .object(["id": .string(record.id.uuidString), "path": .string("/tmp")])
    let invalid = await router.respond(to: try Wire.encoder().encode(request))
    #expect(invalid.error?.code == -32602)
}

@Test func pricingCachePersistsAcrossDatabaseReopen() throws {
    let directory = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: directory) }
    let path = directory.appendingPathComponent("prices.sqlite").path
    do {
        let db = try ScannerDatabase(path: path)
        try db.cachePrices(quote())
        try db.cacheSterlingRate(EuroSterlingRate(poundsPerEuro: Decimal(string: "0.835")!, date: today()))
    }
    let reopened = try ScannerDatabase(path: path)
    #expect(try reopened.cachedPrices("test-001")?.variants.first?.euros == Decimal(string: "0.03"))
    #expect(try reopened.cachedSterlingRate()?.poundsPerEuro == Decimal(string: "0.835"))
}
