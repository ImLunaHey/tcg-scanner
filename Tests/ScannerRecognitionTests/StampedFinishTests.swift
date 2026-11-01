import Foundation
import Testing
import ScannerAPI
@testable import ScannerRecognition

private func stampedCard(_ id: String = "sv02-062", name: String = "Pikachu", total: Int = 193) -> CardCandidate {
    let parts = id.split(separator: "-").map(String.init)
    return CardCandidate(id: id, name: name, localID: parts[1], setID: parts[0], setName: "Base expansion",
        officialSetCount: total, evidence: ["Collector number " + parts[1]], availableFinishes: [.normal, .reverseHolo], category: "Pokemon")
}

@Test func stampedCatalogueUsesSpecialPrintingInsteadOfBaseRarity() {
    let card = stampedCard()
    let edition = CardFinishAnalyzer.resolve(stamp: CardStampEvidence(state: .present), card: card, variants: [])
    #expect(edition.state == .matched && edition.name == "Trick or Trade")
    #expect(edition.variants.allSatisfy { $0.finish == .holo && $0.years == [2023] })
    #expect(CardFinishAnalyzer.decision(edition: edition).suggestedFinish == .holo)
    #expect(TrickOrTradePrintings.entries.count == 90)
    #expect(Set(TrickOrTradePrintings.entries.map(\.product)).count == 90)
    for state: CardStampEvidence.State in [.absent, .uncertain, .unsupported] {
        let uncertain = CardFinishAnalyzer.resolve(stamp: CardStampEvidence(state: state), card: card, variants: [])
        #expect(CardFinishAnalyzer.decision(edition: uncertain).suggestedFinish == nil)
    }
    var wrong = card; wrong.name = "Different card"
    #expect(TrickOrTradePrintings.matches(wrong).isEmpty)
    wrong = card; wrong.officialSetCount = 999
    #expect(TrickOrTradePrintings.matches(wrong).isEmpty)
}

@Test func stampedCatalogueConflictNeedsPositiveIndependentEvidence() throws {
    let card = stampedCard("sv02-097", name: "Mimikyu")
    let variant = CardPrintingVariant(id: "23hmnvtwds1n7f4b2o51f5iqrd8z", finish: .holo, stamps: ["trick-or-trade"])
    let edition = CardFinishAnalyzer.resolve(stamp: CardStampEvidence(state: .present), card: card, variants: [variant])
    #expect(Set(edition.variants.map(\.finish)) == [.normal, .holo])
    #expect(edition.variants.count == 3) // Both market printings and the distinct TCGdex entry survive.
    #expect(CardFinishAnalyzer.decision(edition: edition).uncertaintyReason == .multipleEditionFinishes)
    for state: BorderFinishEvidence.State in [.absent, .unstable, .unsupported, .failed] {
        let result = CardFinishAnalyzer.decision(edition: edition, border: BorderFinishEvidence(state: state, threshold: 0.008213026997467985))
        #expect(result.suggestedFinish == nil)
    }
    let result = CardFinishAnalyzer.decision(edition: edition, border: BorderFinishEvidence(state: .present, threshold: 0.008213026997467985))
    #expect(result.suggestedFinish == .holo && result.edition?.variants == edition.variants)
    #expect(try Wire.decoder().decode(FinishEstimate.self, from: Wire.encoder().encode(result)) == result)
    let reverse = CardPrintingVariant(id: "reverse", finish: .reverseHolo, stamps: ["trick-or-trade"])
    let withReverse = CardFinishAnalyzer.resolve(stamp: CardStampEvidence(state: .present), card: card, variants: [variant, reverse])
    #expect(CardFinishAnalyzer.decision(edition: withReverse,
        border: BorderFinishEvidence(state: .present, threshold: 0.008213026997467985)).suggestedFinish == nil)
}

@Test func unlistedStampCannotUseOrdinaryVariantsOrForceAnEdition() {
    let card = stampedCard("sv09-001", name: "Unlisted", total: 159)
    let ordinary = CardPrintingVariant(id: "regular", finish: .normal, stamps: [])
    let other = CardPrintingVariant(id: "other", finish: .holo, stamps: ["prerelease"])
    let edition = CardFinishAnalyzer.resolve(stamp: CardStampEvidence(state: .present), card: card, variants: [ordinary, other])
    #expect(edition.state == .unlisted && edition.name == nil && edition.variants.isEmpty)
    #expect(CardFinishAnalyzer.decision(edition: edition).suggestedFinish == nil)
    var changed = card; changed.id = "sv09-002"
    #expect(CardFinishAnalyzer.resolve(stamp: CardStampEvidence(state: .present), card: changed,
        variants: [CardPrintingVariant(id: "stamp", finish: .holo, stamps: ["trick-or-trade"])]).state == .uncertain)
}

@Test func editionPricingKeyChangesWhenYearOrStampResolutionChanges() {
    let stamp = CardStampEvidence(state: .present)
    let original = CardPrintingVariant(id: "shared", finish: .holo, stamps: ["trick-or-trade"], years: [2023])
    var year = original; year.years = [2024]
    var differentStamp = original; differentStamp.stamps = ["prerelease"]
    let keys = [original, year, differentStamp].map {
        CardEditionEvidence(state: .matched, candidateID: "sv02-097", stamp: stamp, variants: [$0]).pricingKey
    }
    #expect(Set(keys).count == 3)
}

private struct EmptyPrintings: CardPrintingProviding {
    func printings(cardID: String) async throws -> [CardPrintingVariant] { [] }
}
private struct NoReference: CardReferenceProviding {
    func reference(cardID: String) async throws -> CardReferenceImage? { nil }
}

@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_STAMP_RECORDS"] != nil))
func stampedFinishSavedLibraryKeepsControlsAndResolvesB027() async throws {
    struct Row: Encodable { let batch: Int; let sequence: Int; let result: FinishEstimate }
    let analyzer = CardFinishAnalyzer(catalogue: EmptyPrintings(), references: NoReference())
    let paths = try #require(ProcessInfo.processInfo.environment["SCANNER_STAMP_RECORDS"]).split(separator: ":")
    var rows: [Row] = [], stamps = 0
    for path in paths {
        let record = try Wire.decoder().decode(ScanRecord.self, from: Data(contentsOf: URL(fileURLWithPath: String(path))))
        for identity in record.cardIdentifications ?? [] {
            guard [.reviewRequired, .confirmed].contains(identity.state), var evidence = identity.evidence else { continue }
            if let selected = identity.selectedCardID { evidence.candidates = evidence.candidates.filter { $0.id == selected } }
            let prepared = record.processedImages?.first { $0.id == identity.sourceEditID }
            let source = try #require(identity.sourceEditID == nil ? record.images.first { $0.sequence == identity.request.sequence } : prepared?.image)
            let data = try Data(contentsOf: URL(fileURLWithPath: record.outputDirectory).appendingPathComponent(source.relativePath))
            let result = try await analyzer.analyze(data, evidence: evidence, preparation: prepared?.cardPreparation)
            rows.append(Row(batch: record.batchNumber ?? 0, sequence: identity.request.sequence, result: result))
            if record.batchNumber == 27 {
                stamps += 1
                let ordinal = (identity.request.sequence + 1) / 2
                #expect(result.edition?.stamp.state == .present && result.edition?.name == "Trick or Trade")
                #expect(result.suggestedFinish == ([2, 6].contains(ordinal) ? .holo : .normal))
                #expect(result.edition?.reference?.state == .missing)
                let withoutProvenance = try await analyzer.analyze(data, evidence: evidence, preparation: nil)
                #expect(withoutProvenance.edition == nil)
            } else {
                #expect(result.edition?.stamp.state != .present && result.edition?.stamp.state != .uncertain)
                #expect(result.state == identity.finishEstimate?.state && result.suggestedFinish == identity.finishEstimate?.suggestedFinish,
                    "Existing control B-\(record.batchNumber ?? 0), capture \(identity.request.sequence)")
            }
        }
    }
    #expect(stamps == 6)
    if let output = ProcessInfo.processInfo.environment["SCANNER_STAMP_ANALYSIS_OUTPUT"] {
        try Wire.encoder().encode(rows).write(to: URL(fileURLWithPath: output), options: .withoutOverwriting)
    }
}
