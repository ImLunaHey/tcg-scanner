import Foundation
import Testing
import ScannerAPI
@testable import ScannerRecognition

private func combinedCard(_ finishes: [CardFinish] = [.normal, .holo, .reverseHolo]) -> CardCandidate {
    CardCandidate(id: "sv09-001", name: "Example", localID: "001", setID: "sv09", setName: "Example",
        officialSetCount: 159, evidence: ["Collector number 001"], availableFinishes: finishes, category: "Pokemon")
}
private func border(_ state: BorderFinishEvidence.State) -> BorderFinishEvidence {
    BorderFinishEvidence(state: state, threshold: BorderFoilMeasurement.calibratedThreshold,
        baseline: 0.02, inset: 0.02, downsampled: 0.02)
}
@Test func combinedFinishResolvesOnlyRegularHoloEvidence() throws {
    let card = combinedCard()
    let regular = FinishEstimator.decision(card: card, median: 0.2, dark: 0.7, light: 0.2)
    let resolved = FinishEstimator.combine(regular, card: card, border: border(.present))
    #expect(resolved.state == .estimated && resolved.suggestedFinish == .holo)
    #expect(resolved.uncertaintyReason == nil && resolved.borderEvidence?.state == .present)
    #expect(try Wire.decoder().decode(FinishEstimate.self, from: Wire.encoder().encode(resolved)) == resolved)
    for state: BorderFinishEvidence.State in [.absent, .unstable, .unsupported, .failed] {
        let unresolved = FinishEstimator.combine(regular, card: card, border: border(state))
        #expect(unresolved.state == .uncertain && unresolved.suggestedFinish == nil)
        #expect(unresolved.uncertaintyReason == .multipleRegularFinishes)
    }
}
@Test func combinedFinishKeepsReverseMarkersAndWeakMarkersIndependent() {
    let card = combinedCard()
    let reverse = FinishEstimator.decision(card: card, median: 0.8, dark: 0.2, light: 0.7)
    let weak = FinishEstimator.decision(card: card, median: 0.5, dark: 0.3, light: 0.3)
    let conflict = FinishEstimator.decision(card: card, median: 0.8, dark: 0.2, light: 0.7,
        background: MarkerBackground(darkArea: 0.1, lightArea: 0))
    for state: BorderFinishEvidence.State in [.present, .absent, .unstable, .unsupported, .failed] {
        #expect(FinishEstimator.combine(reverse, card: card, border: border(state)) == reverse)
        #expect(FinishEstimator.combine(weak, card: card, border: border(state)) == weak)
        #expect(FinishEstimator.combine(conflict, card: card, border: border(state)) == conflict)
    }
    var wrongIdentity = card; wrongIdentity.id = "other"
    #expect(FinishEstimator.combine(reverse, card: wrongIdentity, border: border(.present)) == reverse)
}
@Test func combinedFinishAbstainsWhenPositiveTextureConflictsWithCatalogue() {
    let card = combinedCard([.normal, .reverseHolo])
    let marker = FinishEstimator.decision(card: card, median: 0.2, dark: 0.7, light: 0.2)
    let conflict = FinishEstimator.combine(marker, card: card, border: border(.present))
    #expect(conflict.state == .uncertain && conflict.suggestedFinish == nil)
    #expect(conflict.uncertaintyReason == .conflictingFinishEvidence)
    for state: BorderFinishEvidence.State in [.absent, .unstable, .failed] {
        #expect(FinishEstimator.combine(marker, card: card, border: border(state)).suggestedFinish == .normal)
    }
    let holo = combinedCard([.holo, .reverseHolo])
    let knownHolo = FinishEstimator.decision(card: holo, median: 0.2, dark: 0.7, light: 0.2)
    #expect(FinishEstimator.combine(knownHolo, card: holo, border: border(.absent)).suggestedFinish == .holo)
}

// Opt-in saved-image integration: no scanner calls, no network, no file mutations
// except the explicitly requested diagnostics output. Labels are test assertions,
// never passed into FinishEstimator or the production decision rules.
@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_COMBINED_FINISH_RECORDS"] != nil))
func combinedFinishSavedBatchesPreserveUncertaintyAndMatchPhysicalChecks() throws {
    struct Row: Encodable { let batch: Int; let sequence: Int; let name: String; let result: FinishEstimate }
    let paths = try #require(ProcessInfo.processInfo.environment["SCANNER_COMBINED_FINISH_RECORDS"]).split(separator: ":")
    var rows: [Row] = []
    for path in paths {
        let record = try Wire.decoder().decode(ScanRecord.self, from: Data(contentsOf: URL(fileURLWithPath: String(path))))
        var fronts = 0
        for identity in record.cardIdentifications ?? [] {
            guard [.reviewRequired, .confirmed].contains(identity.state), let evidence = identity.evidence else { continue }
            let prepared = record.processedImages?.first(where: { $0.id == identity.sourceEditID })
            let image = try #require(identity.sourceEditID == nil ? record.images.first(where: { $0.sequence == identity.request.sequence }) : prepared?.image)
            let data = try Data(contentsOf: URL(fileURLWithPath: record.outputDirectory).appendingPathComponent(image.relativePath))
            let result = try FinishEstimator.analyze(data, evidence: evidence, preparation: prepared?.cardPreparation)
            let batch = record.batchNumber ?? 0, ordinal = (identity.request.sequence + 1) / 2
            fronts += 1
            rows.append(Row(batch: batch, sequence: identity.request.sequence,
                            name: evidence.candidates.map(\.name).joined(separator: ", "), result: result))
            if evidence.candidates.count == 1, evidence.candidates.first?.id == "sv09-177" {
                #expect(result.suggestedFinish == .holo && result.method == "modern-ex-catalogue-v1")
                #expect(result.markerPolarity == nil && result.borderEvidence == nil)
            }
            if batch == 26 {
                if ordinal == 10 {
                    #expect(result.state == .unsupported && result.suggestedFinish == nil)
                } else {
                    let expected: CardFinish = ordinal <= 9 ? .holo : [12, 19].contains(ordinal) ? .reverseHolo : .normal
                    #expect(result.suggestedFinish == expected, "B-026 C-\(ordinal)")
                }
                if ordinal <= 4 {
                    // Without verified prepared provenance the original marker ambiguity stays.
                    let unprepared = try FinishEstimator.analyze(data, evidence: evidence)
                    #expect(unprepared.suggestedFinish == nil && unprepared.borderEvidence == nil)
                    var unchecked = try #require(prepared?.cardPreparation); unchecked.orientationVerified = false
                    let unverified = try FinishEstimator.analyze(data, evidence: evidence, preparation: unchecked)
                    #expect(unverified.suggestedFinish == nil && unverified.borderEvidence == nil)
                    unchecked.orientationVerified = true; unchecked.orientationBasis = "pokemon-back-pattern"
                    #expect(try FinishEstimator.analyze(data, evidence: evidence, preparation: unchecked).borderEvidence == nil)
                }
            } else if [14, 15, 16, 25].contains(batch) {
                #expect(result.suggestedFinish == nil || result.suggestedFinish == .normal, "Normal control B-\(batch)/C-\(ordinal)")
            }
        }
        if record.batchNumber == 26 { #expect(fronts == 19) }
    }
    if let output = ProcessInfo.processInfo.environment["SCANNER_COMBINED_FINISH_OUTPUT"] {
        try Wire.encoder().encode(rows).write(to: URL(fileURLWithPath: output), options: .withoutOverwriting)
    }
}
