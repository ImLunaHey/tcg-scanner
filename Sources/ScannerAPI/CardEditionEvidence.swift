import Foundation

/// Visual evidence for one specific mark, not proof that a card has no other stamp.
public struct CardStampEvidence: Codable, Sendable, Equatable {
    public enum State: String, Codable, Sendable { case present, absent, uncertain, unsupported }
    public var state: State
    public var kind = "pikachu-pumpkin"
    public var method = "pumpkin-shape-v1"
    public var region: ImageCrop?
    public var sourceQuarterTurns: Int
    /// Similarity to the measured mark shape, not a calibrated probability.
    public var similarity: Double?
    public var baselineSimilarity: Double?
    public var downsampledSimilarity: Double?
    public var evidence: [String]
    public init(state: State, region: ImageCrop? = nil, sourceQuarterTurns: Int = 0,
                similarity: Double? = nil, baselineSimilarity: Double? = nil,
                downsampledSimilarity: Double? = nil, evidence: [String] = []) {
        self.state = state; self.region = region; self.sourceQuarterTurns = sourceQuarterTurns
        self.similarity = similarity; self.baselineSimilarity = baselineSimilarity
        self.downsampledSimilarity = downsampledSimilarity; self.evidence = evidence
    }
}

/// Variant IDs are scoped to source AND base card ID; TCGdex IDs are not globally unique.
public struct CardPrintingVariant: Codable, Sendable, Equatable {
    public var id: String
    public var source: String
    public var finish: CardFinish
    public var stamps: [String]
    public var years: [Int]
    public init(id: String, source: String = "tcgdex", finish: CardFinish, stamps: [String], years: [Int] = []) {
        self.id = id; self.source = source; self.finish = finish; self.stamps = stamps; self.years = years
    }
}

public struct CardEditionEvidence: Codable, Sendable, Equatable {
    public enum State: String, Codable, Sendable { case notDetected, matched, uncertain, unlisted, unavailable }
    public var state: State
    public var candidateID: String?
    public var stamp: CardStampEvidence
    public var name: String?
    public var variants: [CardPrintingVariant]
    public var evidence: [String]
    public var reference: CardReferenceEvidence?
    /// Included in valuation replies so clients cannot display an old ordinary quote after a stamp check.
    public var pricingKey: String
    public init(state: State, candidateID: String?, stamp: CardStampEvidence, name: String? = nil,
                variants: [CardPrintingVariant] = [], evidence: [String] = []) {
        self.state = state; self.candidateID = candidateID; self.stamp = stamp; self.name = name
        self.variants = variants; self.evidence = evidence
        pricingKey = (["edition-v1", candidateID ?? "unknown", state.rawValue, stamp.state.rawValue] +
            variants.map { $0.source + ":" + $0.id + ":" + $0.finish.rawValue + ":" + $0.stamps.sorted().joined(separator: ",") + ":" + $0.years.sorted().map(String.init).joined(separator: ",") }.sorted()).joined(separator: "|")
    }
    public var needsEditionHandling: Bool { stamp.state == .present || stamp.state == .uncertain }
}

/// Comparison with public catalogue artwork; never a labelled physical-finish reference.
public struct CardReferenceEvidence: Codable, Sendable, Equatable {
    public enum State: String, Codable, Sendable { case compared, missing, unavailable }
    public var state: State
    public var url: String?
    public var sha256: String?
    public var stampState: CardStampEvidence.State?
    public var explanation: String
    public init(state: State, url: String? = nil, sha256: String? = nil,
                stampState: CardStampEvidence.State? = nil, explanation: String) {
        self.state = state; self.url = url; self.sha256 = sha256
        self.stampState = stampState; self.explanation = explanation
    }
}
