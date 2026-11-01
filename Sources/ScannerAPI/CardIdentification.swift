import Foundation

public enum IdentificationState: String, Codable, Sendable {
    case processing, reviewRequired, noMatch, failed, confirmed, rejected
}

public enum CardFinish: String, Codable, Sendable, CaseIterable { case unverified, normal, holo, reverseHolo, other }
public enum CardCondition: String, Codable, Sendable, CaseIterable { case unassessed, nearMint, lightlyPlayed, moderatelyPlayed, heavilyPlayed, damaged }

public struct IdentificationRequest: Codable, Sendable, Equatable {
    public var id: UUID
    public var jobID: UUID
    public var sequence: Int
    public var language: String
    public init(id: UUID = UUID(), jobID: UUID, sequence: Int, language: String = "en") {
        self.id = id; self.jobID = jobID; self.sequence = sequence; self.language = language
    }
    public func validate() throws {
        guard (1...100).contains(sequence), language == "en" else {
            throw ScannerError("invalidParams", "Identification currently supports English (en), sequence 1–100.")
        }
    }
}

public struct RecognizedLine: Codable, Sendable, Equatable {
    public var text: String
    public var confidence: Float // Vision transcription confidence, not card identity confidence.
    public var x: Double
    public var y: Double // Normalized, bottom-left origin in the orientation used for OCR.
    public var width: Double
    public var height: Double
    public init(text: String, confidence: Float = 1, x: Double = 0, y: Double = 0, width: Double = 1, height: Double = 0.1) {
        self.text = text; self.confidence = confidence; self.x = x; self.y = y; self.width = width; self.height = height
    }
}

public struct CardCandidate: Codable, Sendable, Equatable {
    public var id: String
    public var name: String
    public var localID: String
    public var setID: String
    public var setName: String
    public var officialSetCount: Int?
    public var evidence: [String]
    public var category: String?
    public var types: [String]?
    public var availableFinishes: [CardFinish]
    public var printingVariants: [CardPrintingVariant]?
    public init(id: String, name: String, localID: String, setID: String, setName: String, officialSetCount: Int? = nil, evidence: [String], availableFinishes: [CardFinish] = [], category: String? = nil, types: [String]? = nil) {
        self.id = id; self.name = name; self.localID = localID; self.setID = setID; self.setName = setName
        self.officialSetCount = officialSetCount; self.evidence = evidence
        self.availableFinishes = availableFinishes; self.category = category; self.types = types
    }
}

public struct RecognitionEvidence: Codable, Sendable {
    public var lines: [RecognizedLine]
    public var candidates: [CardCandidate]
    public var warnings: [String]
    public var orientation: Int
    public init(lines: [RecognizedLine], candidates: [CardCandidate], warnings: [String] = [], orientation: Int = 0) {
        self.lines = lines; self.candidates = candidates; self.warnings = warnings; self.orientation = orientation
    }
}

public struct CardIdentification: Codable, Sendable {
    public var schemaVersion = 1
    public var request: IdentificationRequest
    public var sourceSHA256: String
    public var sourceEditID: UUID?
    public var state: IdentificationState = .processing
    public var createdAt = Date()
    public var completedAt: Date?
    public var reviewedAt: Date?
    public var evidence: RecognitionEvidence?
    public var selectedCardID: String?
    public var error: ScannerError?
    public var method = "apple-vision-tcgdex-text-v1"
    public var finishEstimate: FinishEstimate?
    public var conditionCheck: ConditionCheck?
    public var finish: CardFinish = .unverified
    public var condition: CardCondition = .unassessed
    public var assessmentSource = "manual-review"
    public init(request: IdentificationRequest, sourceSHA256: String) {
        self.request = request; self.sourceSHA256 = sourceSHA256
    }
}
