import Foundation
import Testing
import ScannerAPI
@testable import ScannerRecognition

private let cards = [
    CatalogueCard(id: "swsh3-136", localId: "136", name: "Furret"),
    CatalogueCard(id: "other-136", localId: "136", name: "Furret"),
    CatalogueCard(id: "other-12", localId: "12", name: "Furret"),
    CatalogueCard(id: "other-13", localId: "13", name: "Sentret"),
    CatalogueCard(id: "svp-001", localId: "001", name: "Sprigatito")
]

@Test func nameNumberAndSetTotalProvideIndependentEvidence() {
    let lines = [RecognizedLine(text: "STAGE 1 Furret HP 110", y: 0.88),
                 RecognizedLine(text: "136/189", y: 0.04)]
    let result = CardMatcher.rank(lines: lines, catalogue: cards)
    #expect(result.count == 2) // Reprints remain ambiguous; never select one automatically.
    #expect(result.allSatisfy { $0.card.localId == "136" && $0.totals == [189] })
    #expect(result.allSatisfy { $0.evidence.count == 2 })
}
@Test func nameAloneCannotDistinguishPrintingsAndBodyNamesDoNotMatch() {
    #expect(CardMatcher.rank(lines: [RecognizedLine(text: "Furret", y: 0.9)], catalogue: cards).count == 3)
    #expect(CardMatcher.rank(lines: [RecognizedLine(text: "Furret", y: 0.3)], catalogue: cards).isEmpty)
    #expect(CardMatcher.rank(lines: [RecognizedLine(text: "Furret", confidence: 0.1, y: 0.9)], catalogue: cards).isEmpty)
    #expect(CardMatcher.rank(lines: [RecognizedLine(text: "Furret", y: 0.9), RecognizedLine(text: "99/100", y: 0.01)], catalogue: cards).isEmpty)
}
@Test func prefixesZeroPaddingAndNameBoundariesArePreserved() {
    #expect(CardMatcher.number("TG01") == "TG1")
    #expect(CardMatcher.number("001") == "1")
    #expect(CardMatcher.number("TG01") != CardMatcher.number("001"))
    #expect(CardMatcher.normalized("Farfetch’d") == "farfetch d")
    #expect(CardMatcher.rank(lines: [RecognizedLine(text: "SuperFurret", y: 0.9)], catalogue: cards).isEmpty)
}
@Test func invalidRecognitionRequestsAndImagesAreRejected() throws {
    #expect(throws: ScannerError.self) { try IdentificationRequest(jobID: UUID(), sequence: 0).validate() }
    #expect(throws: ScannerError.self) { try IdentificationRequest(jobID: UUID(), sequence: 1, language: "../en").validate() }
    #expect(throws: ScannerError.self) { try CardOCR.read(Data("not an image".utf8)) }
}

/// Opt-in, read-only catalogue fixture. Never connects to scanner hardware.
@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_TEST_TCGDEX_IMAGE"] != nil))
func recognizeKnownCatalogueImage() async throws {
    let path = try #require(ProcessInfo.processInfo.environment["SCANNER_TEST_TCGDEX_IMAGE"])
    let result = try await CardRecognizer().recognize(Data(contentsOf: URL(fileURLWithPath: path)))
    #expect(result.candidates.contains { $0.id == "swsh3-136" })
    #expect(result.candidates.first?.evidence.contains("Collector number 136") == true)
    #expect(result.candidates.first?.evidence.contains("Printed set total 189") == true)
}

@Test func catalogueCategoryAndTypesAreOptionalForExistingMetadata() throws {
    let json = #"{"id":"sv09-028","localId":"028","name":"Larvesta","set":{"id":"sv09","name":"Journey Together"},"category":"Pokemon","types":["Fire"]}"#
    let detail = try JSONDecoder().decode(CatalogueDetail.self, from: Data(json.utf8))
    #expect(detail.category == "Pokemon" && detail.types == ["Fire"])
    let legacy = #"{"id":"sv09-028","name":"Larvesta","localID":"028","setID":"sv09","setName":"Journey Together","evidence":[],"availableFinishes":[]}"#
    let candidate = try JSONDecoder().decode(CardCandidate.self, from: Data(legacy.utf8))
    #expect(candidate.category == nil && candidate.types == nil)
}
