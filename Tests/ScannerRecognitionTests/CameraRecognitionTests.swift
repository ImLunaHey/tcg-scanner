import Foundation
import Testing
import ScannerAPI
@testable import ScannerRecognition

@Test func cameraFastOrientationRequiresNameNumberAndSetAgreement() {
    var card = CardCandidate(id: "sv01-001", name: "Example", localID: "001", setID: "sv01", setName: "Example", officialSetCount: 198,
        evidence: ["Name in title area", "Collector number 001", "Printed set total 198"])
    func evidence(_ cards: [CardCandidate]) -> RecognitionEvidence { RecognitionEvidence(lines: [], candidates: cards) }
    #expect(CardRecognizer.acceptCameraOrientation(evidence([card])))
    #expect(!CardRecognizer.acceptCameraOrientation(evidence([])))
    #expect(!CardRecognizer.acceptCameraOrientation(evidence([card, card])))
    for missing in card.evidence {
        var partial = card; partial.evidence.removeAll { $0 == missing }
        #expect(!CardRecognizer.acceptCameraOrientation(evidence([partial])))
    }
    card.officialSetCount = nil
    #expect(!CardRecognizer.acceptCameraOrientation(evidence([card])))
}

@Test func cameraFinishKeepsUnsupportedLanguagesAndLayoutsUnresolved() async {
    let card = CardCandidate(id: "swsh11-081", name: "Spectrier", localID: "081", setID: "swsh11", setName: "Lost Origin", officialSetCount: 196,
        evidence: ["Collector number 081"], availableFinishes: [.holo, .reverseHolo])
    let evidence = RecognitionEvidence(lines: [], candidates: [card])
    let analyzer = CardFinishAnalyzer()
    let result = await analyzer.estimate(Data(), evidence: evidence, preparation: nil, camera: true)
    #expect(result.state == .unsupported && result.suggestedFinish == nil && result.borderEvidence == nil)
    let korean = await analyzer.estimate(Data(), evidence: evidence, preparation: nil, language: .korean, camera: true)
    #expect(korean.uncertaintyReason == .languageUnsupported && korean.suggestedFinish == nil)
}

@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_CAMERA_REFERENCE"] != nil))
func cameraOrientationHintMatchesFullOCRAndRecoversWrongHints() async throws {
    let path = try #require(ProcessInfo.processInfo.environment["SCANNER_CAMERA_REFERENCE"])
    let data = try Data(contentsOf: URL(fileURLWithPath: path))
    let recognizer = CardRecognizer()
    let full = try await recognizer.recognize(data, language: .english)
    #expect(full.candidates.count == 1)
    for hint in [full.orientation, (full.orientation + 2) % 4] {
        let result = try await recognizer.recognize(data, language: .english, preferredOrientation: hint)
        #expect(result.candidates.map(\.id) == full.candidates.map(\.id))
        #expect(result.orientation == full.orientation)
    }
}
