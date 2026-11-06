import Foundation

public enum CardGame: String, Codable, Sendable { case pokemon, yugioh, unknown }

public enum CardContentKind: String, Codable, Sendable { case pokemonCodeCard }

/// Per-pair classification from a recognized back, never from a batch default.
public struct CardGameDetection: Codable, Sendable {
    public var number: Int
    public var game: CardGame
    public var contentKind: CardContentKind?
    public var frontSequence: Int?
    public var backSequence: Int?
    public var backSHA256: String?
    public var method: String?
    public var reason: String

    public static func resolve(_ record: ScanRecord) -> [Self] {
        let groups = Dictionary(grouping: record.images, by: { ($0.sequence + 1) / 2 })
        return groups.keys.sorted().map { number in
            let images = groups[number] ?? []
            var result = Self(number: number, game: .unknown, reason: "A complete duplex pair with one recognized back is required.")
            guard record.request.duplex, images.map(\.sequence).sorted() == [number * 2 - 1, number * 2] else { return result }
            let backs: [(OriginalImage, CardGame, String)] = images.compactMap { original in
                guard let edit = record.processedImages?.last(where: { $0.request.sequence == original.sequence && $0.cardPreparation != nil }),
                      edit.sourceSHA256 == original.sha256,
                      let plan = edit.cardPreparation, plan.orientationVerified,
                      let basis = plan.orientationBasis else { return nil }
                switch basis {
                case "pokemon-back-pattern": return (original, .pokemon, basis)
                case "yugioh-back-pattern": return (original, .yugioh, basis)
                default: return nil
                }
            }
            if backs.isEmpty {
                let codeSides = images.filter { original in
                    guard let edit = record.processedImages?.last(where: { $0.request.sequence == original.sequence && $0.cardPreparation != nil }) else { return false }
                    return edit.sourceSHA256 == original.sha256 && edit.cardPreparation?.orientationBasis == "pokemon-code-text"
                }
                if codeSides.count == 2 {
                    result.contentKind = .pokemonCodeCard
                    result.method = "pokemon-code-text-v1"
                    result.reason = "Printed Pokémon online/redemption wording detected on both sides; not a playable card."
                    return result
                }
            }
            guard backs.count == 1, let back = backs.first else {
                if backs.count > 1 { result.reason = "Both captures look like card backs; check pairing." }
                return result
            }
            result.game = back.1; result.backSequence = back.0.sequence
            result.frontSequence = images.first { $0.sequence != back.0.sequence }?.sequence
            result.backSHA256 = back.0.sha256; result.method = back.2
            result.reason = "Game and sides detected from the card back; captures paired in acquisition order."
            return result
        }
    }
}

extension ScanRecord {
    public func detectedGame(for sequence: Int) -> CardGame {
        CardGameDetection.resolve(self).first { $0.number == (sequence + 1) / 2 }?.game ?? .unknown
    }
}
