import Foundation

extension ScannerCoordinator {
    static func metadataMatches(_ remote: StoreCardReply, input: StoreCardInput) -> Bool {
        remote.review == input.review && remote.game == input.game && remote.language == input.language
            && remote.gameSource == input.gameSource && remote.languageSource == input.languageSource
    }

    static func canSync(_ remote: StoreCardReply, input: StoreCardInput, card: StorePlannedCard,
                        baseline: StoreSyncBaseline?) -> Bool {
        // The revision on each subsequent write also protects edits made after
        // this read. A rejected/approved/edited card remains entirely Store-owned.
        guard remote.status == "pending", remote.confirmed == nil, remote.notes == "",
              remote.gameSource != "manual", remote.languageSource != "manual" else { return false }
        let metadataSafe = metadataMatches(remote, input: input)
            || baseline.map { metadataIsScannerOwned(remote, target: input, previous: $0.input) } == true
        let photosSafe = (remote.front == nil && remote.back == nil)
            || photosMatch(remote, card: card)
            || baseline.map { photosMatch(remote, card: $0.card) } == true
        // Target equality permits idempotent recovery after a lost response;
        // baseline equality permits replacing only our own previous values.
        return metadataSafe && photosSafe
    }

    private static func metadataIsScannerOwned(_ remote: StoreCardReply, target: StoreCardInput,
                                              previous: StoreCardInput) -> Bool {
        guard let review = remote.review else { return false }
        func owned<T: Equatable>(_ current: T, _ new: T, _ old: T) -> Bool { current == new || current == old }
        // Compare fields independently so a partially applied repair is also safe
        // to resume, but any third value supplied by a reviewer preserves the card.
        return owned(remote.game, target.game, previous.game)
            && owned(remote.language, target.language, previous.language)
            && owned(remote.gameSource, target.gameSource, previous.gameSource)
            && owned(remote.languageSource, target.languageSource, previous.languageSource)
            && owned(review.recognitionSuggestions, target.review.recognitionSuggestions, previous.review.recognitionSuggestions)
            && owned(review.finishEstimate, target.review.finishEstimate, previous.review.finishEstimate)
            && owned(review.conditionEstimate, target.review.conditionEstimate, previous.review.conditionEstimate)
            && owned(review.borderFindings, target.review.borderFindings, previous.review.borderFindings)
            && owned(review.suggestedPriceGbp, target.review.suggestedPriceGbp, previous.review.suggestedPriceGbp)
    }
}
