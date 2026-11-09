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

    public func estimate(_ data: Data, evidence: RecognitionEvidence, preparation: CardPreparation?, language: CardLanguage = .english, camera: Bool = false) async -> FinishEstimate {
        guard language == .english else { return Self.unsupportedLanguage(evidence: evidence, language: language) }
        do { return try await analyze(data, evidence: evidence, preparation: preparation, language: language, camera: camera) }
        catch {
            let id = evidence.candidates.count == 1 ? evidence.candidates.first?.id : nil
            var result = FinishEstimate(state: .uncertain, candidateID: id,
                evidence: ["Local stamp/finish analysis could not complete."], uncertaintyReason: .analysisFailed)
            result.edition = CardEditionEvidence(state: .unavailable, candidateID: id,
                stamp: CardStampEvidence(state: .uncertain, evidence: ["Stamp analysis was unavailable; no ordinary-printing assumption was made."]))
            return result
        }
    }

    public func analyze(_ data: Data, evidence: RecognitionEvidence, preparation: CardPreparation?, language: CardLanguage = .english, camera: Bool = false) async throws -> FinishEstimate {
        try Task.checkCancellation()
        guard language == .english else { return Self.unsupportedLanguage(evidence: evidence, language: language) }
        // Scanner frames, bad crops and backs cannot establish the expected mark location.
        guard preparation?.orientationVerified == true, preparation?.orientationBasis == "text" else {
            return try await ordinary(data, evidence: evidence, preparation: camera ? nil : preparation)
        }
        let stampTask = Task.detached(priority: .utility) {
            let anniversary = try AnniversaryStampDetector.analyze(data, orientation: evidence.orientation)
            if anniversary.state == .present || anniversary.state == .uncertain { return anniversary }
            return try CardStampDetector.analyze(data, orientation: evidence.orientation)
        }
        let stamp = try await withTaskCancellationHandler { try await stampTask.value } onCancel: { stampTask.cancel() }
        let card = evidence.candidates.count == 1 ? evidence.candidates.first : nil
        if stamp.kind == "pikachu-30th-anniversary" {
            var variants = card?.printingVariants ?? []
            if let card, card.printingVariants == nil { variants = (try? await catalogue.printings(cardID: card.id)) ?? [] }
            return Self.anniversaryDecision(stamp: stamp, card: card, variants: variants)
        }
        if stamp.state == .absent || stamp.state == .unsupported {
            var result = try await ordinary(data, evidence: evidence, preparation: camera ? nil : preparation)
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
        if !camera { edition.reference = try await compareReference(card.id) }
        var border: BorderFinishEvidence?
        let finishes = Set(edition.variants.map(\.finish))
        if !camera, finishes == [.normal, .holo],
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

    static func anniversaryDecision(stamp: CardStampEvidence, card: CardCandidate?, variants: [CardPrintingVariant]) -> FinishEstimate {
        let matches = variants.filter { $0.source == "tcgdex" && !$0.id.isEmpty && Set($0.stamps) == ["30th-anniversary"] && [.normal, .holo, .reverseHolo].contains($0.finish) }
        let numbered = card.map { $0.id == $0.setID + "-" + $0.localID && $0.evidence.contains("Collector number " + $0.localID) } ?? false
        let matched = stamp.kind == "pikachu-30th-anniversary" && stamp.state == .present && numbered && !matches.isEmpty
        let edition = CardEditionEvidence(state: matched ? .matched : .uncertain, candidateID: card?.id, stamp: stamp,
            name: matched ? "30th Anniversary" : nil, variants: matched ? matches : [], evidence: [
                matched ? "The visible 30 mark and independently numbered card match TCGdex's 30th-anniversary variant." : "A visible 30 mark and a matching numbered anniversary catalogue variant are both required."
            ])
        var result = FinishEstimate(state: .uncertain, candidateID: card?.id, evidence: edition.evidence,
            limitations: ["The anniversary mark identifies an edition, not measured foil reflectivity or authenticity."], uncertaintyReason: .editionUnavailable)
        result.method = "anniversary-stamped-printing-v1"; result.edition = edition
        // The current catalogue labels several 30th Pokémon cards normal despite
        // their foil product description. Do not turn that discrepancy into a
        // confident non-holo result. MEE's explicit holo variant remains useful.
        let finishes = Set(matches.map(\.finish))
        if matched && finishes.count == 1 && finishes != [.normal] {
            result.state = .estimated; result.suggestedFinish = finishes.first; result.uncertaintyReason = nil
        } else if matched {
            result.uncertaintyReason = .conflictingFinishEvidence
            result.evidence.append("Anniversary edition resolved; catalogue finish metadata does not establish a reliable physical finish.")
        }
        return result
    }

    private static func unsupportedLanguage(evidence: RecognitionEvidence, language: CardLanguage) -> FinishEstimate {
        var result = FinishEstimate(state: .unsupported,
            candidateID: evidence.candidates.count == 1 ? evidence.candidates.first?.id : nil,
            evidence: ["Automatic finish and stamp checks have not been validated for \(language.displayName) cards."],
            limitations: ["English set-code, border and stamped-printing rules are not applied to other languages. Inspect the physical card for its finish."],
            uncertaintyReason: .languageUnsupported)
        result.method = "language-scope-v1"
        return result
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
