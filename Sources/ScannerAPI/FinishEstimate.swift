import Foundation

public struct FinishEstimate: Codable, Sendable, Equatable {
    public enum State: String, Codable, Sendable { case estimated, uncertain, unsupported }
    public var state: State
    public var suggestedFinish: CardFinish?
    public var candidateID: String?
    public var method = "modern-set-code-polarity-v2"
    public var evidence: [String]
    public var limitations: [String]
    public var region: ImageCrop?
    public var sourceQuarterTurns: Int?
    public var markerPolarity: String?
    public var medianLuminance: Double?
    public var darkBackgroundAreaFraction: Double?
    public var lightBackgroundAreaFraction: Double?
    public init(state: State, suggestedFinish: CardFinish? = nil, candidateID: String? = nil, evidence: [String], limitations: [String] = []) {
        self.state = state; self.suggestedFinish = suggestedFinish; self.candidateID = candidateID
        self.evidence = evidence; self.limitations = limitations
    }
}
