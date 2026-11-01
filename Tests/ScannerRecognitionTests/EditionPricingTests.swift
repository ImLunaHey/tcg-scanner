import Foundation
import Testing
import ScannerAPI
import ScannerCore
import ScannerPersistence
@testable import ScannerRecognition

private struct EditionPricingNoHardware: ScannerBackend {
    func devices() throws -> [Device] { throw ScannerError("test", "Pricing must not access hardware") }
    func inspect(deviceID: String) throws -> Inspection { throw ScannerError("test", "Pricing must not access hardware") }
}
private struct EditionPricingNoNetwork: CardPricingProviding {
    func prices(cardID: String) throws -> CardMarketPrices { throw ScannerError("test", "Unexpected catalogue request") }
    func sterlingRate() throws -> EuroSterlingRate { throw ScannerError("test", "Unexpected rate request") }
}
private let editionCardID = "sv02-097"
private func editionVariant(_ id: String = "pumpkin", source: String = "tcgdex", stamps: [String] = ["trick-or-trade"]) -> CardPrintingVariant {
    CardPrintingVariant(id: id, source: source, finish: .holo, stamps: stamps)
}
private func edition(_ state: CardEditionEvidence.State = .matched,
                     stamp: CardStampEvidence.State = .present, candidateID: String = editionCardID,
                     variants: [CardPrintingVariant] = [editionVariant()]) -> CardEditionEvidence {
    CardEditionEvidence(state: state, candidateID: candidateID, stamp: CardStampEvidence(state: stamp),
                        name: "Trick or Trade", variants: variants)
}
private func editionQuote(_ id: String?, euros: Int, stamps: [String] = []) -> MarketVariantPrice {
    MarketVariantPrice(finish: .holo, euros: Decimal(euros), updated: ISO8601DateFormatter().string(from: Date()),
                       variantID: id, stamps: stamps)
}
private func editionBatch(_ db: ScannerDatabase, editions: [CardEditionEvidence?], manuallyReviewed: Bool = false) throws -> ScanRecord {
    var record = ScanRecord(device: Device(id: "mock", name: "Mock"), request: ScanRequest(duplex: true, prepareCards: true), outputDirectory: "/unused")
    record.state = .completed; record.cardIdentifications = []
    for (offset, edition) in editions.enumerated() {
        for sequence in (offset * 2 + 1)...(offset * 2 + 2) {
            record.images.append(OriginalImage(sequence: sequence, relativePath: "\(sequence).png", bytes: 0,
                sha256: "fixture", width: 100, height: 100, imageCount: 1, uniformType: "public.png"))
        }
        var identity = CardIdentification(request: IdentificationRequest(jobID: record.id, sequence: offset * 2 + 1), sourceSHA256: "fixture")
        identity.state = manuallyReviewed ? .confirmed : .reviewRequired
        identity.selectedCardID = manuallyReviewed ? editionCardID : nil
        identity.finish = manuallyReviewed ? .holo : .unverified
        identity.evidence = RecognitionEvidence(lines: [], candidates: [CardCandidate(id: editionCardID, name: "Mimikyu",
            localID: "097", setID: "sv02", setName: "Paldea Evolved", evidence: ["Collector number 097"], availableFinishes: [.holo, .reverseHolo])])
        identity.finishEstimate = FinishEstimate(state: .estimated, suggestedFinish: .holo, candidateID: editionCardID, evidence: [])
        identity.finishEstimate?.edition = edition
        record.cardIdentifications?.append(identity)
    }
    try db.insert(record, submission: record.request)
    return record
}
private func editionPrices(_ db: ScannerDatabase, quotes: [MarketVariantPrice]) throws -> ScannerCoordinator {
    try db.cachePrices(CardMarketPrices(cardID: editionCardID, variants: quotes))
    try db.cacheSterlingRate(EuroSterlingRate(poundsPerEuro: Decimal(string: "0.80")!, date: String(ISO8601DateFormatter().string(from: Date()).prefix(10))))
    return try ScannerCoordinator(backend: EditionPricingNoHardware(), database: db, cardPricing: EditionPricingNoNetwork())
}

@Test func stampedAndOrdinaryCopiesWithSameFinishUseSeparateQuotes() async throws {
    let db = try ScannerDatabase()
    let mark = edition()
    let record = try editionBatch(db, editions: [nil, mark])
    let service = try editionPrices(db, quotes: [editionQuote("regular", euros: 1),
        editionQuote("pumpkin", euros: 2, stamps: ["trick-or-trade"])])
    let result = try await service.batchValuation(record.id)
    #expect(result.cards.map(\.lowPence) == [80, 160])
    #expect(result.cards.map(\.highPence) == [80, 160])
    #expect(result.cards.map(\.state) == [.priced, .priced])
    #expect(result.cards[0].editionKey == nil && result.cards[1].editionKey == mark.pricingKey)
    #expect(result.cards[0].quotes[0].stamps.isEmpty)
    #expect(result.cards[1].quotes[0].variantID == "pumpkin")
    #expect(result.lowPence == 240 && result.highPence == 240)
    #expect(try Wire.encoder().encode(db.job(record.id)) == Wire.encoder().encode(record))
}

@Test func unresolvedStampedEditionsNeverUseOrdinaryQuotesEvenAfterManualFinishReview() async throws {
    let db = try ScannerDatabase()
    let marks = [edition(.uncertain, stamp: .uncertain), edition(.unlisted, variants: []),
        edition(.unavailable, variants: []), edition(candidateID: "different-card"),
        edition(variants: [editionVariant(source: "tcgplayer")]), edition(variants: [])]
    let record = try editionBatch(db, editions: marks, manuallyReviewed: true)
    let service = try editionPrices(db, quotes: [editionQuote("regular", euros: 1),
        editionQuote("pumpkin", euros: 2, stamps: ["trick-or-trade"])])
    let result = try await service.batchValuation(record.id)
    #expect(result.pricedCount == 0 && result.lowPence == nil)
    #expect(result.cards.allSatisfy { $0.state == .unavailable && $0.quotes.isEmpty })
    #expect(result.cards.map(\.editionKey) == marks.map { Optional($0.pricingKey) })
}

@Test func stampedPricesRequireEveryMatchingVariantAndItsExactStampMetadata() async throws {
    let db = try ScannerDatabase()
    let record = try editionBatch(db, editions: [edition(variants: [editionVariant("one"), editionVariant("two")])])
    let service = try editionPrices(db, quotes: [editionQuote("regular", euros: 1),
        editionQuote("one", euros: 2, stamps: ["trick-or-trade"]),
        editionQuote("two", euros: 3, stamps: ["prerelease"])])
    #expect(try await service.batchValuation(record.id).cards[0].state == .unavailable)
    try db.cachePrices(CardMarketPrices(cardID: editionCardID, variants: [
        editionQuote("one", euros: 2, stamps: ["trick-or-trade"]),
        editionQuote("two", euros: 3, stamps: ["trick-or-trade"])]))
    let result = try await service.batchValuation(record.id)
    #expect(result.cards[0].state == .range)
    #expect(result.lowPence == 160 && result.highPence == 240)
}

@Test func missingStampedQuoteAndStaleCandidateDoNotFallBackToOrdinaryPrice() async throws {
    let db = try ScannerDatabase()
    var record = try editionBatch(db, editions: [edition(), edition()])
    record.cardIdentifications?[1].finishEstimate?.candidateID = "different-card"
    try db.save(record)
    let service = try editionPrices(db, quotes: [editionQuote("regular", euros: 5)])
    let result = try await service.batchValuation(record.id)
    #expect(result.cards.allSatisfy { $0.state == .unavailable && $0.lowPence == nil })
    #expect(result.cards.allSatisfy { $0.editionKey != nil })
}

@Test func providerPreservesStampVariantIDsAndRejectsSharedOrdinaryMarketProducts() throws {
    let data = Data(#"{"id":"sv02-097","variants_detailed":[{"variantId":"regular","type":"holo","size":"standard","pricing":{"cardmarket":{"unit":"EUR","idProduct":1,"trend":1}}},{"variantId":"pumpkin","type":"holo","size":"standard","stamp":["trick-or-trade"],"pricing":{"cardmarket":{"unit":"EUR","idProduct":1,"trend":1}}},{"variantId":"promo","type":"holo","size":"standard","stamp":["prerelease"],"pricing":{"cardmarket":{"unit":"EUR","idProduct":2,"trend":9}}}]}"#.utf8)
    let prices = try JSONDecoder().decode(PricingDetail.self, from: data).quotes()
    #expect(prices.variants.count == 3)
    #expect(prices.variants[0].euros == nil && prices.variants[1].euros == nil)
    #expect(prices.variants[1].variantID == "pumpkin" && prices.variants[1].stamps == ["trick-or-trade"])
    #expect(prices.variants[2].euros == 9)
}
