import Foundation

/// Ephemeral camera lookup. It has no job, batch, image ID or persistence fields.
public struct CameraIdentification: Codable, Sendable {
    public enum State: String, Codable, Sendable { case noCard, cardBack, codeCard, matched, ambiguous, noMatch }
    public var schemaVersion = 1
    public var state: State
    public var game: CardGame?
    public var candidates: [CardCandidate]
    public var uncataloguedCard: UncataloguedCard?
    public var energySymbol: EnergySymbolEvidence?
    public var finishEstimate: FinishEstimate?
    public init(state: State, game: CardGame? = nil, evidence: RecognitionEvidence? = nil) {
        self.state = state; self.game = game; candidates = evidence?.candidates ?? []
        uncataloguedCard = evidence?.uncataloguedCard; energySymbol = evidence?.energySymbol
    }
    public static let maximumImageBytes = 512 * 1024
    public static let maximumRequestBytes = 768 * 1024
}
