import Foundation
import Testing
import ScannerAPI
@testable import ScannerRecognition

@Test func starWarsIdentifiersRequireLanguageSetAndCollectorFraction() {
    let ids = StarWarsPrintedText.identifiers(["ASH•EN R 206/264"])
    #expect(ids.count == 1 && ids.first?.set == "ASH" && ids.first?.number == "206")
    #expect(StarWarsPrintedText.identifiers(["ASH", "206/264"]).isEmpty)
    #expect(StarWarsPrintedText.identifiers(["ASH EN 206/2640"]).isEmpty)
    #expect(StarWarsPrintedText.identifiers(["ASH EN 686"]).first?.number == "686")
    #expect(StarWarsPrintedText.identifiers(["ASH FR 206/264"]).isEmpty)
    #expect(StarWarsPrintedText.identifiers(["ASH EN 206/264", "SOR EN 20/252"]).count == 2)
}
@Test func starWarsMatchingRejectsConflictingNamesAndTotals() throws {
    let id = try #require(StarWarsPrintedText.identifiers(["ASH EN 206/264"]).first)
    let set = StarWarsSet(setId: "ASH", fullName: "Ashes of the Empire", numberCards: 264)
    let card = StarWarsCard(Set: "ASH", Number: "206", Name: "Kelleran Beq", Type: "Unit")
    let lines = [RecognizedLine(text: "Kelleran Beq")]
    let result = StarWarsMatcher.match(lines: lines, id: id, set: set, cards: [card])
    #expect(result.first?.provider == "swu-db")
    #expect(StarWarsMatcher.match(lines: [RecognizedLine(text: "Other Card")], id: id, set: set, cards: [card]).isEmpty)
    #expect(StarWarsMatcher.match(lines: lines, id: id, set: StarWarsSet(setId: "ASH", fullName: "Wrong", numberCards: 880), cards: [card]).isEmpty)
    #expect(StarWarsMatcher.match(lines: lines, id: id, set: set, cards: [card,card]).isEmpty)
}
@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_SWU_RECOGNIZE"] != nil))
func savedStarWarsCatalogueMatches() async throws {
    let recognizer = StarWarsRecognizer()
    let expected = ["206", "192", "044", "046", "686", "256", "173", "215", "181", "418", "048", "136", "237", "080", "025", "007"]
    for sequence in stride(from: 2, through: 32, by: 2) {
        let path = "/tmp/swu-prepared-\(sequence).png"
        let data = try Data(contentsOf: URL(fileURLWithPath: path))
        let r = try await recognizer.recognize(data, language: .english)
        #expect(r.candidates.map(\.id) == ["swu-db:ASH-" + expected[sequence / 2 - 1]])
        print("SWU MATCH",sequence,r.candidates.map { $0.id + " " + $0.name })
        if r.candidates.isEmpty { print("SWU OCR",sequence,r.lines.map(\.text)) }
    }
}
