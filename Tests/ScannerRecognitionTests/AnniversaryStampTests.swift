import Foundation
import Testing
import ScannerAPI
@testable import ScannerRecognition

@Test func savedAnniversaryStampsAgreeAtTwoResolutions() throws {
    guard let path = ProcessInfo.processInfo.environment["SCANNER_ANNIVERSARY_FIXTURES"] else { return }
    struct Item: Decodable { let label: String; let path: String; let present: Bool }
    for item in try JSONDecoder().decode([Item].self, from: Data(contentsOf: URL(fileURLWithPath: path))) {
        let result = try AnniversaryStampDetector.analyze(Data(contentsOf: URL(fileURLWithPath: item.path)), orientation: 0)
        print("ANNIVERSARY",item.label,result.state.rawValue,result.baselineSimilarity ?? 0,result.downsampledSimilarity ?? 0)
        #expect((result.state == .present) == item.present, "\(item.label)")
    }
}

@Test func anniversaryEditionRequiresVisibleMarkAndNumberedVariant() {
    var stamp = CardStampEvidence(state: .present); stamp.kind = "pikachu-30th-anniversary"
    let card = CardCandidate(id: "mee-009", name: "Basic Grass Energy", localID: "009", setID: "mee", setName: "Mega Evolution Energy", evidence: ["Collector number 009"])
    let variant = CardPrintingVariant(id: "anniversary", finish: .holo, stamps: ["30th-anniversary"])
    let resolved = CardFinishAnalyzer.anniversaryDecision(stamp: stamp, card: card, variants: [variant])
    #expect(resolved.edition?.name == "30th Anniversary" && resolved.edition?.state == .matched)
    #expect(resolved.suggestedFinish == .holo)
    #expect(CardFinishAnalyzer.anniversaryDecision(stamp: stamp, card: nil, variants: [variant]).edition?.state == .uncertain)
    #expect(CardFinishAnalyzer.anniversaryDecision(stamp: stamp, card: card, variants: []).suggestedFinish == nil)
    stamp.state = .uncertain
    #expect(CardFinishAnalyzer.anniversaryDecision(stamp: stamp, card: card, variants: [variant]).suggestedFinish == nil)
    stamp.state = .present
    let contradictory = CardPrintingVariant(id: "normal", finish: .normal, stamps: ["30th-anniversary"])
    #expect(CardFinishAnalyzer.anniversaryDecision(stamp: stamp, card: card, variants: [contradictory]).suggestedFinish == nil)
}
