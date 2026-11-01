import Foundation
import Testing
import ScannerAPI
@testable import ScannerRecognition

private func modernEx() -> CardCandidate {
    CardCandidate(id: "sv09-177", name: "Salamence ex", localID: "177", setID: "sv09",
        setName: "Journey Together", officialSetCount: 159, evidence: ["Collector number 177"],
        availableFinishes: [.holo], category: "Pokemon")
}

@Test func modernExUsesExactCataloguePrintingWithoutClaimingImageMeasurements() throws {
    var missingCategory = modernEx(); missingCategory.category = nil
    var ordinaryNumber = modernEx(); ordinaryNumber.localID = "011"; ordinaryNumber.id = "sv09-011"
    ordinaryNumber.evidence = ["Collector number 011"]
    for card in [modernEx(), missingCategory, ordinaryNumber] {
        // A catalogue inference does not need pixels or pretend that they were measured.
        let result = try FinishEstimator.analyze(Data(), evidence: RecognitionEvidence(lines: [], candidates: [card]))
        #expect(result.state == .estimated && result.suggestedFinish == .holo)
        #expect(result.candidateID == card.id && result.method == "modern-ex-catalogue-v1")
        #expect(result.markerPolarity == nil && result.borderEvidence == nil && result.region == nil)
        #expect(result.uncertaintyReason == nil)
        #expect(try Wire.decoder().decode(FinishEstimate.self, from: Wire.encoder().encode(result)) == result)
    }
}

@Test func catalogueFinishAbstainsForOtherPrintingsAndIncompleteEvidence() throws {
    var variants: [CardCandidate] = []
    for finishes: [CardFinish] in [[], [.normal], [.holo, .normal], [.holo, .reverseHolo]] {
        var card = modernEx(); card.availableFinishes = finishes; variants.append(card)
    }
    for set in ["pop4", "svp", "swsh12", "me01", "sv09promo"] {
        var card = modernEx(); card.setID = set; card.id = set + "-177"; variants.append(card)
    }
    var wrongID = modernEx(); wrongID.id = "sv09-176"; variants.append(wrongID)
    var noNumber = modernEx(); noNumber.evidence = ["Name in title area only; printing is ambiguous"]; variants.append(noNumber)
    var wrongNumber = modernEx(); wrongNumber.evidence = ["Collector number 176"]; variants.append(wrongNumber)
    var noTotal = modernEx(); noTotal.officialSetCount = nil; variants.append(noTotal)
    var zeroTotal = modernEx(); zeroTotal.officialSetCount = 0; variants.append(zeroTotal)
    var plainName = modernEx(); plainName.name = "Salamence"; variants.append(plainName)
    for number in ["0", "-1", "+177", "177a"] {
        var card = modernEx(); card.localID = number; card.id = "sv09-" + number
        card.evidence = ["Collector number " + number]; variants.append(card)
    }
    for card in variants {
        let result = FinishEstimator.estimate(Data(), evidence: RecognitionEvidence(lines: [], candidates: [card]))
        #expect(result.state != .estimated && result.suggestedFinish == nil, "Unexpected catalogue inference for \(card)")
    }
    for candidates in [[], [modernEx(), modernEx()]] {
        let result = try FinishEstimator.analyze(Data(), evidence: RecognitionEvidence(lines: [], candidates: candidates))
        #expect(result.state == .uncertain && result.uncertaintyReason == .ambiguousIdentity)
    }
}

@Test func energyNeedsValidatedFoilExamplesEvenWithNormalCatalogueVariant() throws {
    for number in ["002", "010"] {
        let card = CardCandidate(id: "sve-" + number, name: "Fire Energy", localID: number, setID: "sve",
            setName: "Scarlet & Violet Energy", officialSetCount: 24,
            evidence: ["Collector number " + number], availableFinishes: [.normal, .reverseHolo], category: "Energy")
        let result = try FinishEstimator.analyze(Data(), evidence: RecognitionEvidence(lines: [], candidates: [card]))
        #expect(result.state == .unsupported && result.suggestedFinish == nil)
        #expect(result.evidence.first?.contains("Basic Energy foil variants need validation") == true)
        #expect(result.markerPolarity == nil && result.borderEvidence == nil)
    }
}
