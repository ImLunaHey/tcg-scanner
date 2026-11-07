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

@Test func invertedFooterCannotPromoteFlavourTextToCardName() {
    let catalogue = [CatalogueCard(id: "col1-6", localId: "6", name: "Groudon")]
    let inverted = [RecognizedLine(text: "Kyogre", y: 0.05, height: 0.04),
                    RecognizedLine(text: "battled ferociously against Groudon.", y: 0.94, height: 0.02),
                    RecognizedLine(text: "026/103", y: 0.94, height: 0.02)]
    #expect(CardMatcher.rank(lines: inverted, catalogue: catalogue).isEmpty)
    #expect(CardMatcher.footerOrientationScore(inverted) == 0)
    let upright = inverted.map { line in
        RecognizedLine(text: line.text, y: 1 - line.y - line.height, height: line.height)
    }
    #expect(CardMatcher.footerOrientationScore(upright) == 1)
    #expect(CardMatcher.rank(lines: upright, catalogue: catalogue).isEmpty)
}

@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_B051_FIXTURE"] != nil))
func missingPrintingDoesNotMatchUpsideDownFlavourText() async throws {
    let path = try #require(ProcessInfo.processInfo.environment["SCANNER_B051_FIXTURE"])
    let result = try await CardRecognizer().recognize(Data(contentsOf: URL(fileURLWithPath: path)))
    #expect(result.orientation == 0)
    #expect(result.candidates.isEmpty)
    #expect(result.lines.contains { $0.text.contains("026/103") && $0.y < 0.3 })
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

@Test func uncertainEXSuffixRequiresCollectorNumberAndHorizontalTitle() {
    let catalogue = [CatalogueCard(id: "sv09-177", localId: "177", name: "Salamence ex"), CatalogueCard(id: "old-64", localId: "64", name: "Salamence")]
    let title = RecognizedLine(text: "Salamence @X", y: 0.908, height: 0.052)
    let footer = RecognizedLine(text: "177/159", y: 0.032, height: 0.017)
    #expect(CardMatcher.rank(lines: [title, footer], catalogue: catalogue).map(\.card.id) == ["sv09-177"])
    #expect(!CardMatcher.rank(lines: [title], catalogue: catalogue).contains { $0.card.id == "sv09-177" })
    #expect(CardMatcher.rank(lines: [RecognizedLine(text: "Salamence CX", y: 0.386, height: 0.427)], catalogue: catalogue).isEmpty)
}
@Test func energySymbolTitleRequiresPrintedSetCodeAndUniqueNumber() {
    let catalogue = [CatalogueCard(id: "sve-010", localId: "010", name: "Fire Energy"), CatalogueCard(id: "sve-002", localId: "002", name: "Fire Energy"), CatalogueCard(id: "other-010", localId: "010", name: "Water Energy")]
    let title = RecognizedLine(text: "Basic Energy", y: 0.9)
    let code = RecognizedLine(text: "SVEEN", y: 0.03, height: 0.02)
    let number = RecognizedLine(text: "010", y: 0.02, height: 0.02)
    #expect(CardMatcher.rank(lines: [title, code, number], catalogue: catalogue).map(\.card.id) == ["sve-010"])
    #expect(CardMatcher.rank(lines: [title, number], catalogue: catalogue).isEmpty)
    #expect(CardMatcher.rank(lines: [title, code, number, RecognizedLine(text: "002", y: 0.02, height: 0.02)], catalogue: catalogue).isEmpty)
}
@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_RETRY_FIXTURE"] != nil))
func savedEnergyAndFullArtEXAreIdentifiedWithoutFeeding() async throws {
    let root = URL(fileURLWithPath: try #require(ProcessInfo.processInfo.environment["SCANNER_RETRY_FIXTURE"]))
    let record = try Wire.decoder().decode(ScanRecord.self, from: Data(contentsOf: root.appendingPathComponent("manifest.json")))
    let recognizer = CardRecognizer()
    for (sequence, expected) in [(2, "sve-010"), (12, "sv09-177")] {
        let image = try #require(record.processedImages?.last { $0.request.sequence == sequence && $0.cardPreparation != nil })
        let evidence = try await recognizer.recognize(Data(contentsOf: root.appendingPathComponent(image.image.relativePath)))
        #expect(evidence.candidates.map(\.id) == [expected])
        #expect(evidence.orientation == 0)
    }
}

@Test func rarityDotIsNotADigitAndConflictingSetTotalsStayDistinct() {
    let catalogue = [CatalogueCard(id: "sv09-028", localId: "028", name: "Larvesta")]
    let title = RecognizedLine(text: "Larvesta", y: 0.91, height: 0.04)
    let whole = CardMatcher.rank(lines: [title, RecognizedLine(text: "028/1590", y: 0.03, height: 0.02)], catalogue: catalogue)
    let focused = CardMatcher.rank(lines: [title, RecognizedLine(text: "028/159•", y: 0.03, height: 0.02)], catalogue: catalogue)
    #expect(whole.first?.totals == [1590]) // Never silently drop the last digit.
    #expect(focused.first?.totals == [159])
    #expect(CardMatcher.rank(lines: [title, RecognizedLine(text: "029/159•", y: 0.03, height: 0.02)], catalogue: catalogue).isEmpty)
}

@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_LARVESTA_FIXTURE"] != nil))
func savedLarvestaFooterIsRereadFromPixelsWithoutChangingTheCard() async throws {
    let path = try #require(ProcessInfo.processInfo.environment["SCANNER_LARVESTA_FIXTURE"])
    let data = try Data(contentsOf: URL(fileURLWithPath: path))
    let initial = try #require(CardOCR.read(data).first)
    #expect(initial.lines.contains { $0.text == "028/1590" })
    let footer = try CardOCR.refineFooter(data, reading: initial)
    #expect(footer.lines.contains { $0.text.contains("028/159") && !$0.text.contains("1590") })
    #expect(footer.lines.filter { $0.text.contains("028/159") }.allSatisfy { $0.y < 0.15 && $0.height < 0.1 })
    let result = try await CardRecognizer().recognize(data)
    #expect(result.candidates.map(\.id) == ["sv09-028"])
    #expect(result.candidates.first?.evidence.contains("Printed set total 159") == true)
    #expect(result.warnings.contains { $0.contains("re-read") })
}

@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_REPEAT_BATCH"] != nil))
func savedRepeatBatchMatchesAllElevenExpectedPrintings() async throws {
    let root = URL(fileURLWithPath: try #require(ProcessInfo.processInfo.environment["SCANNER_REPEAT_BATCH"]))
    let record = try Wire.decoder().decode(ScanRecord.self, from: Data(contentsOf: root.appendingPathComponent("manifest.json")))
    let recognizer = CardRecognizer()
    let expected = ["sve-010", "sv09-028", "sv09-015", "sv09-099", "sv09-149", "sv09-100", "sv09-076", "sv09-177", "sv09-139", "sv09-071", "sv09-120"]
    for (index, id) in expected.enumerated() {
        let image = try #require(record.processedImages?.last { $0.request.sequence == (index + 1) * 2 && $0.cardPreparation != nil })
        let result = try await recognizer.recognize(Data(contentsOf: root.appendingPathComponent(image.image.relativePath)))
        #expect(result.candidates.map(\.id) == [id])
    }
}

@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_PREPARED_ORIENTATION_FIXTURE"] != nil))
func deskewedMixedBatchKeepsAllElevenPrintingMatches() async throws {
    let root = URL(fileURLWithPath: try #require(ProcessInfo.processInfo.environment["SCANNER_PREPARED_ORIENTATION_FIXTURE"]))
    let recognizer = CardRecognizer()
    let expected = ["sve-010", "sv09-015", "sv09-028", "sv09-099", "sv09-149", "sv09-100", "sv09-076", "sv09-177", "sv09-139", "sv09-071", "sv09-120"]
    for (index, id) in expected.enumerated() {
        let data = try Data(contentsOf: root.appendingPathComponent(String(format: "C-%03d.png", (index + 1) * 2)))
        let result = try await recognizer.recognize(data)
        #expect(result.candidates.map(\.id) == [id])
        #expect(result.orientation == 0)
    }
}

@Test func footerRecoveryRequiresAgreementWithoutTrimmingNumbers() {
    let catalogue = [CatalogueCard(id: "example-042", localId: "042", name: "Binacle")]
    let title = RecognizedLine(text: "Binacle", y: 0.91, height: 0.04)
    func ranks(_ number: String) -> [CardMatcher.Ranked] {
        CardMatcher.rank(lines: [title, RecognizedLine(text: number, y: 0.03, height: 0.02)], catalogue: catalogue)
    }
    #expect(CardMatcher.agreeingFooterRanks(ranks("042/088"), ranks("042/088 0")).first?.totals == [88])
    #expect(CardMatcher.agreeingFooterRanks(ranks("042/088"), ranks("042/0880")).isEmpty)
    #expect(CardMatcher.agreeingFooterRanks(ranks("042/088"), ranks("043/088")).isEmpty)
    #expect(CardMatcher.agreeingFooterRanks(ranks("042/0880"), ranks("042/0880")).first?.totals == [880])
    #expect(CardMatcher.agreeingFooterRanks(ranks(""), ranks("")).isEmpty)
}

@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_BINACLE_FIXTURE"] != nil))
func savedOutlinedRarityCircleSeparatesFromSetTotal() async throws {
    let path = try #require(ProcessInfo.processInfo.environment["SCANNER_BINACLE_FIXTURE"])
    let data = try Data(contentsOf: URL(fileURLWithPath: path))
    let initial = try #require(CardOCR.read(data).first)
    #expect(initial.lines.contains { $0.text == "042/0880" })
    for brightness in [0.15, 0.30] {
        let footer = try CardOCR.refineFooter(data, reading: initial, brightness: brightness)
        #expect(footer.lines.contains { $0.text == "042/088" || $0.text == "042/088 0" })
    }
    let result = try await CardRecognizer().recognize(data)
    #expect(result.candidates.map(\.id) == ["me03-042"])
    #expect(result.candidates.first?.evidence.contains("Printed set total 88") == true)
    #expect(result.warnings.contains { $0.contains("re-read") })
}
