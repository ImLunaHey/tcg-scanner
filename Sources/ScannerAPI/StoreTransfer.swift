import Foundation

public struct StoreTransferStatus: Codable, Sendable {
    public enum State: String, Codable, Sendable { case notSent, sending, sent, failed }
    public var schemaVersion = 1
    public var jobID: UUID
    public var configured: Bool
    public var state: State = .notSent
    public var totalCards = 0
    public var sentCards = 0
    public var reviewURL: String?
    public var error: ScannerError?
    public var updatedAt = Date()
    public init(jobID: UUID, configured: Bool) { self.jobID = jobID; self.configured = configured }
}
