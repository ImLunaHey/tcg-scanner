import Foundation
import ScannerAPI

public struct StoreImport: Codable, Sendable {
    public var scannerInstanceId: String
    public var sourceBatchId: UUID
    public var batchNumber: String
    public var cards: [StoreCardInput]
}
public struct StoreCardInput: Codable, Sendable {
    public var cardNumber: String
    public var review: StoreReview
    public var game: String? = nil
    public var language: String? = nil
    public var gameSource: String? = nil
    public var languageSource: String? = nil
    enum CodingKeys: String, CodingKey { case cardNumber, review, game, language, gameSource, languageSource }
    public func encode(to encoder: any Encoder) throws {
        var values = encoder.container(keyedBy: CodingKeys.self)
        try values.encode(cardNumber, forKey: .cardNumber); try values.encode(review, forKey: .review)
        try values.encode(game, forKey: .game); try values.encode(language, forKey: .language)
        try values.encode(gameSource, forKey: .gameSource); try values.encode(languageSource, forKey: .languageSource)
    }
}
public struct StoreReview: Codable, Sendable {
    public struct Suggestion: Codable, Sendable {
        public var provider: String? = nil
        public var externalId: String? = nil
        public var name: String
        public var setName: String
        public var collectorNumber: String
        public var language: String
    }
    public var recognitionSuggestions: [Suggestion]
    public var finishEstimate: String?
    public var conditionEstimate: String?
    public var borderFindings: [String]
    public var suggestedPriceGbp: Int?
}
public struct StoreBatchReply: Codable, Sendable {
    public var id: UUID
    public var scannerInstanceId: String
    public var sourceBatchId: UUID
    public var batchNumber: String
    public var reviewUrl: String
}
public struct StorePhoto: Codable, Sendable {
    public var id: UUID
    public var sha256: String
    public var sizeBytes: Int
}
public struct StoreCardReply: Codable, Sendable {
    public var id: UUID
    public var scanBatchId: UUID
    public var cardNumber: String
    public var revision: Int
    public var status: String
    public var front: StorePhoto?
    public var back: StorePhoto?
}
public struct StoreImportReply: Codable, Sendable {
    public var batch: StoreBatchReply
    public var cards: [StoreCardReply]
}
/// Only staging operations: intentionally no approval or inventory-writing method.
public protocol StoreStaging: Sendable {
    var origin: String { get }
    var scannerInstanceID: String { get }
    func importBatch(_ value: StoreImport) async throws -> StoreImportReply
    func upload(card: StoreCardReply, front: Data, back: Data) async throws -> StoreCardReply
}

struct StorePlannedCard: Codable, Sendable {
    var number: Int
    var front: OriginalImage
    var back: OriginalImage
}
struct StoreTransferLedger: Codable, Sendable {
    var status: StoreTransferStatus
    var origin: String
    var snapshot: ScanRecord
    var submission: StoreImport
    var cards: [StorePlannedCard]
}
