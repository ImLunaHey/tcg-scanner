import Foundation

/// Visible scan observations, never a selling grade. Regions use normalized
/// top-left coordinates in the identification's exact source image.
public struct ConditionCheck: Codable, Sendable, Equatable {
    public enum State: String, Codable, Sendable { case checked, unsupported, inconclusive }
    public struct Finding: Codable, Sendable, Equatable {
        public enum Kind: String, Codable, Sendable { case edgeWhitening, cornerWhitening, linearMark }
        public var kind: Kind
        public var region: ImageCrop
        public var explanation: String
        public init(kind: Kind, region: ImageCrop, explanation: String) {
            self.kind = kind; self.region = region; self.explanation = explanation
        }
    }
    public var method = "blue-back-visible-marks-v3"
    public var state: State
    public var findings: [Finding]
    public var limitations: [String]
    public var analyzedWidth: Int?
    public var analyzedHeight: Int?
    public var backRecognition: String?
    public var coverage: [String]?
    public init(state: State, findings: [Finding] = [], limitations: [String]) {
        self.state = state; self.findings = findings; self.limitations = limitations
    }
}
