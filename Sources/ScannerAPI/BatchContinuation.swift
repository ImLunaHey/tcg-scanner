import Foundation

public enum ScanLimits {
    public static let imagesPerFeed = 100
    public static let imagesPerBatch = 1000
    public static let imageSequences = 1...imagesPerBatch
    public static let responseBytes = 32 * 1024 * 1024
}

/// A subsequent physical feed keeps the batch identity and prior card numbers.
public struct BatchContinuation: Codable, Sendable {
    public var requestID: UUID
    public var firstSequence: Int
    public var createdAt: Date
    public var previousCompletedAt: Date?
    public var state: ScanState
    public var startedAt: Date?
    public var completedAt: Date?
    public var imagesSaved: Int
    public init(requestID: UUID, record: ScanRecord) {
        self.requestID = requestID; firstSequence = record.images.count + 1
        createdAt = Date(); previousCompletedAt = record.completedAt
        state = .queued; imagesSaved = 0
    }
}
