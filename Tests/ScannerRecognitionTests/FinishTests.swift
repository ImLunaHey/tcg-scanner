import Foundation
import Testing
import ScannerAPI
@testable import ScannerRecognition

@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_FINISH_FIXTURES"] != nil))
func inspectSavedFinishEvidenceWithoutChangingReviews() throws {
    let files = try #require(ProcessInfo.processInfo.environment["SCANNER_FINISH_FIXTURES"]).split(separator: ":")
    for file in files {
        let record = try Wire.decoder().decode(ScanRecord.self, from: Data(contentsOf: URL(fileURLWithPath: String(file))))
        for identity in record.cardIdentifications ?? [] {
            guard let evidence = identity.evidence, evidence.candidates.count == 1,
                  let image = record.processedImages?.first(where: { $0.id == identity.sourceEditID })?.image else { continue }
            let data = try Data(contentsOf: URL(fileURLWithPath: record.outputDirectory).appendingPathComponent(image.relativePath))
            let result = try FinishEstimator.analyze(data, evidence: evidence)
            let known: [Int: CardFinish] = record.batchNumber == 9 ? [4: .reverseHolo, 18: .normal] :
                record.batchNumber == 13 ? [2: .normal, 4: .normal, 6: .normal, 8: .normal, 10: .holo, 18: .holo] :
                record.batchNumber == 14 ? Dictionary(uniqueKeysWithValues: (1...15).map { ($0 * 2, CardFinish.normal) }) :
                record.batchNumber == 15 ? Dictionary(uniqueKeysWithValues: (1...14).map { ($0 * 2, CardFinish.normal) }) : [:]
            if let expected = known[identity.request.sequence] {
                // Preserve the calibrated detections and resolve all normal B-014 examples.
                if record.batchNumber == 14 || record.batchNumber == 15 || record.batchNumber == 9 || identity.request.sequence != 6 {
                    #expect(result.suggestedFinish == expected)
                } else {
                    #expect(result.suggestedFinish == nil || result.suggestedFinish == expected)
                }
            }
            print("FINISH", record.batchNumber ?? 0, identity.request.sequence, evidence.candidates[0].name,
                  String(data: try Wire.encoder().encode(result), encoding: .utf8)!)
        }
    }
}

private func finishCard(_ finishes: [CardFinish] = [.normal, .reverseHolo]) -> CardCandidate {
    CardCandidate(id: "sv09-028", name: "Example", localID: "028", setID: "sv09", setName: "Example", officialSetCount: 159,
                  evidence: ["Collector number 028"], availableFinishes: finishes)
}
@Test func finishDecisionSeparatesInferenceFromAmbiguity() {
    #expect(FinishEstimator.decision(card: finishCard(), median: 0.8, dark: 0.2, light: 0.7).suggestedFinish == .reverseHolo)
    #expect(FinishEstimator.decision(card: finishCard(), median: 0.2, dark: 0.7, light: 0.2).suggestedFinish == .normal)
    #expect(FinishEstimator.decision(card: finishCard([.holo, .reverseHolo]), median: 0.2, dark: 0.7, light: 0.2).suggestedFinish == .holo)
    let ambiguous = FinishEstimator.decision(card: finishCard([.normal, .holo, .reverseHolo]), median: 0.2, dark: 0.7, light: 0.2)
    #expect(ambiguous.state == .uncertain && ambiguous.suggestedFinish == nil)
    // A blank patch or weak contrast cannot establish a printed marker.
    for (median, dark, light) in [(0.5, 0.3, 0.3), (0.9, 0.0, 1.0), (0.1, 1.0, 0.0)] {
        #expect(FinishEstimator.decision(card: finishCard(), median: median, dark: dark, light: light).suggestedFinish == nil)
    }
}
@Test func unsupportedFinishLayoutsAndAmbiguousIdentityAbstain() throws {
    let bytes = Data()
    for candidates in [[], [finishCard(), finishCard()]] {
        let result = try FinishEstimator.analyze(bytes, evidence: RecognitionEvidence(lines: [], candidates: candidates))
        #expect(result.state == .uncertain && result.suggestedFinish == nil)
    }
    var old = finishCard(); old.setID = "swsh3"
    var promo = finishCard(); promo.setID = "svp"
    var fullArt = finishCard(); fullArt.localID = "177"
    for card in [old, promo, fullArt, finishCard([.holo])] {
        let result = try FinishEstimator.analyze(bytes, evidence: RecognitionEvidence(lines: [], candidates: [card]))
        #expect(result.state == .unsupported && result.suggestedFinish == nil)
    }
}


@Test func markerBackgroundSeparatesDenseLettersFromClearPatches() {
    let w = 50, h = 24
    // Dense text makes whole-box brightness inconclusive. A clear header patch
    // remains wider than a glyph stem and taller than the printed outline.
    var pixels = [Double](repeating: 0.5, count: w * h)
    for y in 2..<8 { for x in 30..<45 { pixels[y * w + x] = 0.1 } }
    for y in 9..<22 { for x in stride(from: 5, to: 45, by: 5) {
        for dx in 0..<3 { pixels[y * w + x + dx] = 0.9 }
    } }
    let background = MarkerBackground.measure(pixels, width: w, height: h)
    #expect(background.polarity == "dark")
    #expect(FinishEstimator.decision(card: finishCard(), median: 0.49, dark: 0.35, light: 0.35,
                                     background: background).suggestedFinish == .normal)
    let inverted = MarkerBackground.measure(pixels.map { 1 - $0 }, width: w, height: h)
    #expect(inverted.polarity == "light")
    #expect(FinishEstimator.decision(card: finishCard(), median: 0.51, dark: 0.35, light: 0.35,
                                     background: inverted).suggestedFinish == .reverseHolo)
    #expect(FinishEstimator.decision(card: finishCard([.normal, .holo, .reverseHolo]),
        median: 0.49, dark: 0.35, light: 0.35, background: background).suggestedFinish == nil)
    #expect(FinishEstimator.decision(card: finishCard(), median: 0.8, dark: 0.2, light: 0.7,
                                     background: background).suggestedFinish == nil)
}

@Test func markerBackgroundRejectsOutlinesAndCompetingPatches() {
    let w = 50, h = 24
    var pixels = [Double](repeating: 0.5, count: w * h)
    for x in 0..<w { pixels[x] = 0.1; pixels[(h - 1) * w + x] = 0.1 }
    for y in 0..<h { pixels[y * w] = 0.1; pixels[y * w + w - 1] = 0.1 }
    #expect(MarkerBackground.measure(pixels, width: w, height: h).polarity == nil)
    for y in 5..<11 { for x in 5..<20 { pixels[y * w + x] = 0.1 } }
    for y in 13..<19 { for x in 25..<40 { pixels[y * w + x] = 0.9 } }
    #expect(MarkerBackground.measure(pixels, width: w, height: h).polarity == nil)
    for blank in [0.1, 0.9] {
        let background = MarkerBackground.measure(Array(repeating: blank, count: w * h), width: w, height: h)
        #expect(FinishEstimator.decision(card: finishCard(), median: blank, dark: blank < 0.5 ? 1 : 0,
            light: blank > 0.5 ? 1 : 0, background: background).suggestedFinish == nil)
    }
}
