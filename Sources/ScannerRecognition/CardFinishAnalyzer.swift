import Foundation
import ScannerAPI

/// Shared by automatic scanning and saved-image refresh. Images stay on this Mac;
/// catalogue metadata and public reference artwork are the only network traffic.
public actor CardFinishAnalyzer {
    private let catalogue: any CardPrintingProviding
    private let references: any CardReferenceProviding
    public init(catalogue: any CardPrintingProviding = CardPrintingProvider(),
                references: any CardReferenceProviding = TCGdexReferenceCache()) {
        self.catalogue = catalogue; self.references = references
    }

    public func estimate(_ data: Data, evidence: RecognitionEvidence, preparation: CardPreparation?) async -> FinishEstimate {
        do { return try await analyze(data, evidence: evidence, preparation: preparation) }
        catch {
            let id = evidence.candidates.count == 1 ? evidence.candidates.first?.id : nil
            var result = FinishEstimate(state: .uncertain, candidateID: id,
                evidence: ["Local stamp/finish analysis could not complete."], uncertaintyReason: .analysisFailed)
            result.edition = CardEditionEvidence(state: .unavailable, candidateID: id,
                stamp: CardStampEvidence(state: .uncertain, evidence: ["Stamp analysis was unavailable; no ordinary-printing assumption was made."]))
            return result
        }
    }

    public func analyze(_ data: Data, evidence: RecognitionEvidence, preparation: CardPreparation?) async throws -> FinishEstimate {
        try Task.checkCancellation()
        // Scanner frames, bad crops and backs cannot establish the expected mark location.
        guard preparation?.orientationVerified == true, preparation?.orientationBasis == "text" else {
            return try await ordinary(data, evidence: evidence, preparation: preparation)
        }
        let stampTask = Task.detached(priority: .utility) { try CardStampDetector.analyze(data, orientation: evidence.orientation) }
        let stamp = try await withTaskCancellationHandler { try await stampTask.value } onCancel: { stampTask.cancel() }
        let card = evidence.candidates.count == 1 ? evidence.candidates.first : nil
        if stamp.state == .absent || stamp.state == .unsupported {
            var result = try await ordinary(data, evidence: evidence, preparation: preparation)
            result.edition = CardEditionEvidence(state: stamp.state == .absent ? .notDetected : .unavailable,
                candidateID: card?.id, stamp: stamp)
            return result
        }
        guard stamp.state == .present, let card,
              card.evidence.contains("Collector number " + card.localID) else {
            var result = FinishEstimate(state: .uncertain, candidateID: card?.id,
                evidence: ["A clear pumpkin mark and a single numbered printing are required before inferring its stamped finish."],
                uncertaintyReason: stamp.state == .uncertain ? .stampUncertain : .ambiguousIdentity)
            result.edition = CardEditionEvidence(state: .uncertain, candidateID: card?.id, stamp: stamp)
            return result
        }
        var variants = card.printingVariants ?? []
        var warnings: [String] = []
        if card.printingVariants == nil {
            do { variants = try await catalogue.printings(cardID: card.id) }
            catch is CancellationError { throw CancellationError() }
            catch { warnings.append("Live TCGdex printing metadata was unavailable; only bundled catalogue evidence can be used.") }
        }
        try Task.checkCancellation()
        var edition = Self.resolve(stamp: stamp, card: card, variants: variants)
        edition.evidence += warnings
        // Reference artwork is not finish-labelled. Compare only the presence of
        // the same mark; it must never become a normal/holo/reverse training label.
        edition.reference = try await compareReference(card.id)
        var border: BorderFinishEvidence?
        let finishes = Set(edition.variants.map(\.finish))
        if finishes == [.normal, .holo],
           card.setID.range(of: #"^(sv|me)[0-9]{2}(\.[0-9][a-z]?)?$"#, options: .regularExpression) != nil {
            let task = Task.detached(priority: .utility) { try BorderFoilAnalyzer.analyze(data, orientation: evidence.orientation) }
            do {
                let value = try await withTaskCancellationHandler { try await task.value } onCancel: { task.cancel() }
                border = BorderFinishEvidence(state: BorderFinishEvidence.State(rawValue: value.state.rawValue)!,
                    threshold: value.threshold, baseline: value.baseline, inset: value.inset, downsampled: value.downsampled)
            } catch is CancellationError { throw CancellationError() }
            catch { border = BorderFinishEvidence(state: .failed, threshold: BorderFoilMeasurement.calibratedThreshold) }
        }
        return Self.decision(edition: edition, border: border)
    }

    private func ordinary(_ data: Data, evidence: RecognitionEvidence, preparation: CardPreparation?) async throws -> FinishEstimate {
        let task = Task.detached(priority: .utility) { try FinishEstimator.analyze(data, evidence: evidence, preparation: preparation) }
        return try await withTaskCancellationHandler { try await task.value } onCancel: { task.cancel() }
    }

    static func resolve(stamp: CardStampEvidence, card: CardCandidate, variants: [CardPrintingVariant]) -> CardEditionEvidence {
        guard stamp.state == .present, card.id == card.setID + "-" + card.localID,
              card.evidence.contains("Collector number " + card.localID) else {
            return CardEditionEvidence(state: .uncertain, candidateID: card.id, stamp: stamp,
                evidence: ["A clear mark and consistent numbered catalogue identity are required."])
        }
        let stamped = variants.filter { $0.source == "tcgdex" && !$0.id.isEmpty && Set($0.stamps) == ["trick-or-trade"] && [.normal, .holo, .reverseHolo].contains($0.finish) }
        let matches = stamped + TrickOrTradePrintings.matches(card)
        return CardEditionEvidence(state: matches.isEmpty ? .unlisted : .matched, candidateID: card.id, stamp: stamp,
            name: matches.isEmpty ? nil : "Trick or Trade", variants: matches,
            evidence: matches.isEmpty ? ["A pumpkin stamp is visible, but no supported stamped catalogue printing matches this base card."] :
                ["The visible pumpkin mark and numbered base card match stamped catalogue printings.",
                 "Listed years are catalogue candidates, not proof of the physical release year. Coverage is limited to known variants and the 2022–2024 market registry."])
    }

    static func decision(edition: CardEditionEvidence, border: BorderFinishEvidence? = nil) -> FinishEstimate {
        var result = FinishEstimate(state: .uncertain, candidateID: edition.candidateID,
            evidence: edition.evidence,
            limitations: ["Catalogue-based stamped-printing estimate, not measured reflectivity or authenticity. Metadata can be incomplete or incorrect.",
                "The same mark can be reused. Exact release year and other promotional stamp families are not established."],
            uncertaintyReason: .editionUnavailable)
        result.method = "pumpkin-stamped-printing-v1"; result.edition = edition; result.borderEvidence = border
        guard edition.stamp.state == .present, edition.state == .matched else { return result }
        let finishes = Set(edition.variants.map(\.finish))
        if finishes.count == 1, let finish = finishes.first {
            result.state = .estimated; result.suggestedFinish = finish; result.uncertaintyReason = nil
            result.evidence.append("All matching stamped catalogue entries agree on this finish; the ordinary-printing finish was not substituted.")
        } else if finishes == [.normal, .holo], border?.state == .present {
            result.state = .estimated; result.suggestedFinish = .holo; result.uncertaintyReason = nil
            result.evidence.append("Stamped catalogue entries disagree, but the prepared modern card has stable positive foil-border evidence in all three checks, supporting holo.")
            result.limitations.append("Border texture was calibrated on a limited modern-card sample. It is not a calibrated probability.")
        } else {
            result.uncertaintyReason = .multipleEditionFinishes
            result.evidence.append("Matching stamped catalogue entries list different finishes. Missing or unstable foil texture cannot establish normal.")
        }
        return result
    }

    private func compareReference(_ id: String) async throws -> CardReferenceEvidence {
        do {
            guard let reference = try await references.reference(cardID: id) else {
                return CardReferenceEvidence(state: .missing, explanation: "TCGdex has no image reference for this base card.")
            }
            let task = Task.detached(priority: .utility) { try CardStampDetector.analyze(reference.data, orientation: 0) }
            let stamp = try await withTaskCancellationHandler { try await task.value } onCancel: { task.cancel() }
            return CardReferenceEvidence(state: .compared, url: reference.url.absoluteString, sha256: reference.sha256,
                stampState: stamp.state, explanation: stamp.state == .absent ?
                    "The pumpkin mark detected in the scan is absent from the locally cached catalogue image. This comparison checks the mark, not the whole artwork identity, finish or release year." :
                    "The catalogue image does not provide a clear unstamped control. Its mark check is retained as evidence; it cannot establish a finish or release year.")
        } catch is CancellationError { throw CancellationError() }
        catch {
            return CardReferenceEvidence(state: .unavailable, explanation: "Catalogue image comparison was unavailable; no finish was inferred from a missing reference.")
        }
    }
}
