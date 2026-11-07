import Foundation
import Testing
import ScannerAPI
@testable import ScannerRecognition

private let printedLines = [
    RecognizedLine(text: "Kyogre", y: 0.90, height: 0.045),
    RecognizedLine(text: "HP150", y: 0.90, height: 0.05),
    RecognizedLine(text: "BASIC", y: 0.93, height: 0.03),
    RecognizedLine(text: "battled ferociously against Groudon.", y: 0.04, height: 0.02),
    RecognizedLine(text: "DLR EN", y: 0.05, height: 0.02),
    RecognizedLine(text: "026/103 •", y: 0.04, height: 0.02)
]
@Test func printedDetailsRemainSeparateFromCatalogueIdentity() throws {
    let result = try #require(PrintedCardDetails.read(printedLines))
    #expect(result.name == "Kyogre" && result.setCode == "DLR" && result.collectorNumber == "026/103")
    #expect(result.setName == "Delta Reign")
    #expect(try Wire.decoder().decode(UncataloguedCard.self, from: Wire.encoder().encode(result)) == result)
}
@Test func unreadableOrConflictingPrintedDetailsDoNotBecomePendingPrintings() {
    #expect(PrintedCardDetails.read(Array(printedLines.dropLast())) == nil)
    #expect(PrintedCardDetails.read(printedLines + [RecognizedLine(text: "027/103", y: 0.04, height: 0.02)]) == nil)
    #expect(PrintedCardDetails.read(printedLines + [RecognizedLine(text: "ABC EN", y: 0.05, height: 0.02)]) == nil)
    #expect(PrintedCardDetails.read(printedLines.map {
        RecognizedLine(text: $0.text, y: 1 - $0.y - $0.height, height: $0.height)
    }) == nil)
}

@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_B051_RECORD"] != nil))
func pendingBatchDetailsHandleJoinedSetCodesAndHealthNumbers() async throws {
    let path = try #require(ProcessInfo.processInfo.environment["SCANNER_B051_RECORD"])
    let record = try Wire.decoder().decode(ScanRecord.self, from: Data(contentsOf: URL(fileURLWithPath: path)))
    let recognizer = CardRecognizer()
    for (sequence, name, number) in [(2, "Kyogre", "026/103"), (4, "Inkay", "113/103"), (42, "Growlithe", "018/103")] {
        let prepared = try #require(record.processedImages?.last { $0.request.sequence == sequence })
        let data = try Data(contentsOf: URL(fileURLWithPath: record.outputDirectory).appendingPathComponent(prepared.image.relativePath))
        let evidence = try await recognizer.recognize(data)
        #expect(evidence.candidates.isEmpty)
        #expect(evidence.uncataloguedCard?.name == name)
        #expect(evidence.uncataloguedCard?.setCode == "DLR")
        #expect(evidence.uncataloguedCard?.collectorNumber == number)
    }
}
