import Foundation

public struct FinishEstimate: Codable, Sendable, Equatable {
    public enum State: String, Codable, Sendable { case estimated, uncertain, unsupported }
    public enum UncertaintyReason: String, Codable, Sendable {
        case ambiguousIdentity, markerNotFound, markerContrast, conflictingContrast, multipleRegularFinishes, missingRegularFinish, analysisFailed, conflictingFinishEvidence
        case stampUncertain, editionUnavailable, multipleEditionFinishes
    }
    public var state: State
    public var suggestedFinish: CardFinish?
    public var candidateID: String?
    public var method = "modern-set-code-and-border-v5"
    public var borderEvidence: BorderFinishEvidence?
    public var edition: CardEditionEvidence?
    public var uncertaintyReason: UncertaintyReason?
    public var evidence: [String]
    public var limitations: [String]
    public var region: ImageCrop?
    public var sourceQuarterTurns: Int?
    public var markerPolarity: String?
    public var medianLuminance: Double?
    public var darkBackgroundAreaFraction: Double?
    public var lightBackgroundAreaFraction: Double?
    public init(state: State, suggestedFinish: CardFinish? = nil, candidateID: String? = nil, evidence: [String], limitations: [String] = [], uncertaintyReason: UncertaintyReason? = nil) {
        self.state = state; self.suggestedFinish = suggestedFinish; self.candidateID = candidateID
        self.evidence = evidence; self.limitations = limitations
        self.uncertaintyReason = uncertaintyReason
    }
}

/// Measured image evidence, never a physical finish confirmation or probability.
public struct BorderFinishEvidence: Codable, Sendable, Equatable {
    public enum State: String, Codable, Sendable { case present, absent, unstable, unsupported, failed }
    public var state: State
    public var threshold: Double
    public var baseline: Double?
    public var inset: Double?
    public var downsampled: Double?
    public var method = "independent-border-bands-v1"
    public init(state: State, threshold: Double, baseline: Double? = nil, inset: Double? = nil, downsampled: Double? = nil) {
        self.state = state; self.threshold = threshold; self.baseline = baseline
        self.inset = inset; self.downsampled = downsampled
    }
}
