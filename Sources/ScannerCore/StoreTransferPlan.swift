import Foundation
import ScannerAPI

extension StoreTransferLedger {
    /// The current Store scan-staging contract has no printing-variant field.
    /// Base identity or a reviewed finish cannot preserve a detected stamp there.
    static func validateEditionSupport(record: ScanRecord) throws {
        for identity in (record.cardIdentifications ?? []).sorted(by: { $0.request.sequence < $1.request.sequence }) {
            guard identity.finishEstimate?.edition?.needsEditionHandling == true else { continue }
            let number = record.request.duplex ? (identity.request.sequence + 1) / 2 : identity.request.sequence
            throw ScannerError("storeNotReady", "C-\(BatchNaming.number(number)) has a detected or uncertain stamp. Store must support stamped-edition metadata before this card can be sent; its ordinary printing will not be substituted.")
        }
    }

    static func make(record: ScanRecord, store: any StoreStaging, valuation: BatchValuation?) throws -> Self {
        guard record.state == .completed, record.request.duplex, let batch = record.batchNumber,
              !record.images.isEmpty, record.images.count.isMultiple(of: 2), record.images.count <= 100 else {
            throw ScannerError("storeNotReady", "Send a completed, numbered duplex card batch with both sides of every card.")
        }
        try validateEditionSupport(record: record)
        let images = record.images.sorted { $0.sequence < $1.sequence }
        guard images.map(\.sequence) == Array(1...images.count) else { throw ScannerError("storeNotReady", "Batch capture order is incomplete.") }
        var cards: [StorePlannedCard] = [], inputs: [StoreCardInput] = []
        for offset in stride(from: 0, to: images.count, by: 2) {
            let number = offset / 2 + 1
            guard record.detectedGame(for: images[offset].sequence) == .pokemon else {
                throw ScannerError("unsupportedGame", "C-\(BatchNaming.number(number)) is not a detected Pokémon pair. Store export for other games is not supported yet.")
            }
            let pair = Array(images[offset...offset + 1])
            let edits = try pair.map { original -> ProcessedImage in
                let label = "C-\(BatchNaming.number(number)), capture \(original.sequence)"
                guard let edit = record.processedImages?.last(where: { $0.request.sequence == original.sequence && $0.cardPreparation != nil }) else {
                    throw ScannerError("storeNotReady", "\(label) has no prepared image. Run card preparation first.")
                }
                guard edit.cardPreparation?.orientationVerified == true else {
                    throw ScannerError("storeNotReady", "\(label) has uncertain orientation. Refresh card preparation and identification before sending.")
                }
                guard edit.image.uniformType == "public.png" else {
                    throw ScannerError("storeNotReady", "\(label) must be prepared as PNG.")
                }
                guard edit.image.bytes > 0, edit.image.bytes <= 10 * 1024 * 1024,
                      (100...8000).contains(edit.image.width), (100...8000).contains(edit.image.height) else {
                    throw ScannerError("storeNotReady", "\(label) exceeds Store's prepared PNG limits: 10 MiB and 100–8000 pixels per axis.")
                }
                if let identity = record.cardIdentifications?.first(where: { $0.request.sequence == original.sequence }) {
                    guard identity.state != .processing, identity.sourceSHA256 == edit.image.sha256 else {
                        throw ScannerError("storeNotReady", "C-\(BatchNaming.number(number)) has unfinished or outdated checks. Refresh its identification before sending.")
                    }
                    guard identity.request.language == record.request.resolvedCardLanguage.rawValue else {
                        throw ScannerError("storeNotReady", "C-\(BatchNaming.number(number)) has identification evidence for a different card language. Refresh its identification before sending.")
                    }
                }
                return edit
            }
            let identities = edits.map { edit in record.cardIdentifications?.first { $0.request.sequence == edit.request.sequence } }
            let matches = (0..<2).filter { index in
                guard let identity = identities[index] else { return false }
                return [.confirmed, .reviewRequired].contains(identity.state) && !(identity.evidence?.candidates.isEmpty ?? true)
            }
            let backs = (0..<2).filter { edits[$0].cardPreparation?.orientationBasis == "pokemon-back-pattern" }
            let front: Int
            if matches.count == 1, !backs.contains(matches[0]) { front = matches[0] }
            else if matches.isEmpty, backs.count == 1 { front = 1 - backs[0] }
            else { throw ScannerError("storeNotReady", "C-\(BatchNaming.number(number)) has uncertain front/back sides. Review the pair before sending.") }
            let identity = identities[front]
            let candidates = [.confirmed, .reviewRequired].contains(identity?.state ?? .noMatch) ? (identity?.evidence?.candidates ?? []) : []
            let suggestions = candidates.prefix(10).map { StoreReview.Suggestion(provider: "tcgdex", externalId: $0.id, name: $0.name, setName: $0.setName, collectorNumber: $0.localID, language: identity?.request.language) }
            // Store uses these observations to prefill condition notes. Capture
            // warnings and empty-check summaries describe the scan, not the card.
            var findings: [String] = []
            if let check = identities[1 - front]?.conditionCheck, check.state == .checked {
                for finding in check.findings {
                    let note = String(("Back: " + finding.explanation).prefix(500))
                    if !findings.contains(note) { findings.append(note) }
                }
            }
            let finish = identity.flatMap { item -> String? in
                if item.finish != .unverified { return item.finish.rawValue }
                guard record.request.resolvedCardLanguage == .english,
                      item.finishEstimate?.state == .estimated,
                      item.finishEstimate?.candidateID == ScannerCoordinator.pricedCandidate(item)?.id else { return nil }
                return item.finishEstimate?.suggestedFinish?.rawValue
            }
            let currentCardID = identity.flatMap { ScannerCoordinator.pricedCandidate($0)?.id }
            let quote = valuation?.cards.first {
                record.request.resolvedCardLanguage == .english &&
                $0.number == number && currentCardID != nil && $0.cardID == currentCardID &&
                $0.editionKey == identity?.finishEstimate?.edition?.pricingKey
            }
            let price = quote?.state == .priced && quote?.stale == false && quote?.lowPence == quote?.highPence ? quote?.lowPence : nil
            inputs.append(StoreCardInput(cardNumber: "C-" + BatchNaming.number(number), review: StoreReview(
                recognitionSuggestions: suggestions, finishEstimate: finish, conditionEstimate: nil,
                borderFindings: Array(findings.prefix(30)), suggestedPriceGbp: price),
                game: "pokemon", language: record.request.resolvedCardLanguage.rawValue, gameSource: "recognition", languageSource: "scannerConfiguration"))
            cards.append(StorePlannedCard(number: number, front: edits[front].image, back: edits[1 - front].image))
        }
        var status = StoreTransferStatus(jobID: record.id, configured: true)
        status.totalCards = cards.count
        return Self(status: status, origin: store.origin, snapshot: record,
            submission: StoreImport(scannerInstanceId: store.scannerInstanceID, sourceBatchId: record.id,
                batchNumber: BatchNaming.batchFolder(batch), cards: inputs), cards: cards)
    }
}
