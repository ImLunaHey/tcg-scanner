import Foundation
import Testing
import ScannerAPI
@testable import ScannerRecognition

private let printedLines = [
    RecognizedLine(text: "Kyogre", y: 0.90, height: 0.045),
    RecognizedLine(text: "150", y: 0.90, height: 0.05),
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
