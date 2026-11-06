import ScannerAPI

/// A corroborating cue cannot invent a card, bypass number checks or infer foil.
enum RarityEvidence {
    static func expectedSymbol(_ rarity: String) -> String? {
        switch rarity.lowercased() {
        case "common": "circle"
        case "uncommon": "diamond"
        case "rare", "rare holo", "holo rare": "star"
        case "double rare": "doubleStar"
        default: nil // Metallic stars, promos and special rarities are not modelled.
        }
    }
    static func apply(_ reading: RaritySymbolDetector.Reading?, to candidates: [CardCandidate]) -> [CardCandidate] {
        guard let reading else { return candidates }
        let scored: [(card: CardCandidate, rank: Int, index: Int)] = candidates.enumerated().map { index, candidate in
            var card = candidate
            var rank = 0
            if CardMatcher.number(card.localID) == CardMatcher.number(reading.collectorNumber),
               card.officialSetCount == Int(reading.setTotal),
               let rarity = card.rarities?.first, let expected = expectedSymbol(rarity) {
                if reading.symbol == expected {
                    card.evidence.append("Printed black rarity symbol agrees with catalogue rarity: " + rarity)
                    rank = 1
                } else {
                    card.evidence.append("Rarity symbol conflict: printed \(reading.symbol), catalogue \(rarity); review identity")
                    rank = -1
                }
            }
            return (card, rank, index)
        }
        return scored.sorted { $0.rank != $1.rank ? $0.rank > $1.rank : $0.index < $1.index }.map(\.card)
    }
}
