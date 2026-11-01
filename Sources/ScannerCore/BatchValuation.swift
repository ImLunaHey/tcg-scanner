import Foundation
import ScannerAPI

extension ScannerCoordinator {
    public func batchValuation(_ id: UUID) async throws -> BatchValuation {
        let record = try await scanStatus(id)
        let paired = record.request.duplex && (record.request.prepareCards == true || record.processedImages?.contains(where: { $0.cardPreparation != nil }) == true)
        let groups = Dictionary(grouping: record.images, by: { paired ? ($0.sequence + 1) / 2 : $0.sequence })
        let identities = record.cardIdentifications ?? []
        var selected: [Int: (CardIdentification, CardCandidate)] = [:]
        for (number, images) in groups {
            let matches = identities.filter { identity in
                images.contains { $0.sequence == identity.request.sequence }
                    && [.confirmed, .reviewRequired].contains(identity.state)
                    && !(identity.evidence?.candidates.isEmpty ?? true)
            }
            // Two plausible fronts or a missing duplex side cannot safely represent one card.
            if matches.count == 1, !paired || images.count == 2, let card = Self.pricedCandidate(matches[0]) {
                selected[number] = (matches[0], card)
            }
        }
        let snapshot = try await pricingCache.snapshot(Set(selected.values.map { $0.1.id }))
        let cards = groups.keys.sorted().map { number -> CardValuation in
            guard let (identity, card) = selected[number] else {
                return CardValuation(number: number, reason: "A single card match and complete pair are needed.")
            }
            return Self.value(number: number, identity: identity, card: card, snapshot: snapshot)
        }
        return BatchValuation(batchID: id, cards: cards, exchangeRate: snapshot.rate,
            refreshing: !snapshot.pending.isEmpty || snapshot.ratePending)
    }
    static func pricedCandidate(_ identity: CardIdentification) -> CardCandidate? {
        if identity.state == .confirmed {
            return identity.evidence?.candidates.first { $0.id == identity.selectedCardID }
        }
        if identity.state == .reviewRequired, identity.evidence?.candidates.count == 1 { return identity.evidence?.candidates.first }
        return nil
    }
    private static func value(number: Int, identity: CardIdentification, card: CardCandidate, snapshot: PricingCache.Snapshot) -> CardValuation {
        var result = CardValuation(number: number, reason: "Price unavailable.")
        result.cardID = card.id
        let edition = identity.finishEstimate?.edition
        result.editionKey = edition?.pricingKey
        var editionVariants: [CardPrintingVariant]?
        if let edition, edition.needsEditionHandling {
            guard edition.candidateID == card.id, identity.finishEstimate?.candidateID == card.id else {
                result.reason = "Refresh stamped-edition evidence for the selected card before pricing."; return result
            }
            guard edition.stamp.state == .present, edition.state == .matched, !edition.variants.isEmpty else {
                result.reason = "The stamped edition must be resolved before pricing; ordinary-printing prices are not substituted."; return result
            }
            guard edition.variants.allSatisfy({ $0.source == "tcgdex" && !$0.id.isEmpty && !$0.stamps.isEmpty }) else {
                result.reason = "No matching TCGdex stamped-variant price is available."; return result
            }
            editionVariants = edition.variants
        }
        guard let prices = snapshot.prices[card.id], let rate = snapshot.rate else {
            if snapshot.pending.contains(card.id) || snapshot.ratePending { result.state = .pending; result.reason = "Loading Cardmarket prices and GBP rate." }
            else { result.reason = "Cardmarket price or GBP rate unavailable; will retry automatically." }
            return result
        }
        guard prices.cardID == card.id else { result.reason = "Cached catalogue price does not match this card."; return result }
        let now = Date()
        guard now.timeIntervalSince(prices.fetchedAt) < 7 * 86400, now.timeIntervalSince(rate.fetchedAt) < 7 * 86400,
              rate.poundsPerEuro > 0, rate.poundsPerEuro < 10 else { result.reason = "Cached prices or exchange rate are too old."; return result }
        guard !prices.requiresPrintingReview else { result.reason = "Edition must be identified before pricing this card."; return result }
        let finish: CardFinish?
        if identity.finish != .unverified { finish = identity.finish }
        else if identity.finishEstimate?.state == .estimated, identity.finishEstimate?.candidateID == card.id { finish = identity.finishEstimate?.suggestedFinish }
        else { finish = nil }
        let finishes: [CardFinish]
        if let finish { finishes = [finish] }
        else if let editionVariants { finishes = Array(Set(editionVariants.map(\.finish))) }
        else if identity.finishEstimate?.candidateID == card.id, identity.finishEstimate?.uncertaintyReason == .multipleRegularFinishes { finishes = [.normal, .holo] }
        else { finishes = card.availableFinishes.filter { [.normal, .holo, .reverseHolo].contains($0) } }
        guard !finishes.isEmpty, finishes.allSatisfy({ [.normal, .holo, .reverseHolo].contains($0) }) else {
            result.reason = "Finish needs identification before pricing."; return result
        }
        let quotes: [MarketVariantPrice]
        if let editionVariants {
            let possible = editionVariants.filter { finishes.contains($0.finish) }
            func matches(_ quote: MarketVariantPrice, _ variant: CardPrintingVariant) -> Bool {
                quote.variantID == variant.id && quote.finish == variant.finish && Set(quote.stamps) == Set(variant.stamps)
            }
            guard !possible.isEmpty,
                  possible.allSatisfy({ variant in prices.variants.contains { matches($0, variant) } }) else {
                result.reason = "No reliable price for every matching stamped variant."; return result
            }
            quotes = prices.variants.filter { quote in possible.contains { matches(quote, $0) } }
        } else {
            quotes = prices.variants.filter { $0.stamps.isEmpty && finishes.contains($0.finish) }
        }
        guard finishes.allSatisfy({ finish in quotes.contains { $0.finish == finish } }),
              !quotes.isEmpty, quotes.allSatisfy({ $0.euros.map { !$0.isNaN && $0 > 0 && $0 <= 1_000_000 } == true }) else {
            result.reason = editionVariants == nil ? "No reliable trend price for every possible finish or printing." :
                "No reliable price for every matching stamped variant."; return result
        }
        let pence = quotes.compactMap { quote -> Int? in
            guard let euros = quote.euros else { return nil }
            var amount = euros * rate.poundsPerEuro * 100
            var rounded = Decimal()
            NSDecimalRound(&rounded, &amount, 0, .plain)
            return NSDecimalNumber(decimal: rounded).intValue
        }
        result.lowPence = pence.min(); result.highPence = pence.max(); result.quotes = quotes
        result.state = finish == nil || quotes.count > 1 ? .range : .priced
        if editionVariants != nil {
            result.reason = result.state == .range ? "Range across matching stamped variants." : "Cardmarket trend for the matched stamped variant."
        } else {
            result.reason = result.state == .range ? "Range across possible finishes or foil patterns." : "Cardmarket trend for the matched finish."
        }
        result.stale = now.timeIntervalSince(prices.fetchedAt) > PricingCache.ttl || now.timeIntervalSince(rate.fetchedAt) > PricingCache.ttl
        let formatter = ISO8601DateFormatter()
        formatter.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
        if quotes.contains(where: { quote in
            guard let text = quote.updated, let date = formatter.date(from: text) ?? ISO8601DateFormatter().date(from: text) else { return true }
            return now.timeIntervalSince(date) > 3 * 86400
        }) { result.stale = true }
        return result
    }
}
