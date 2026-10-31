import Foundation

public struct ScanProgress: Codable, Sendable {
    public var imagesSaved: Int
    public var totalImages: Int? // ADF total is unknown until completion.
    public init(imagesSaved: Int) { self.imagesSaved = imagesSaved }
}
public struct ScanProfile: Codable, Sendable, Equatable {
    public var id: UUID
    public var name: String
    public var dpi: Int
    public var duplex: Bool
    public var format: OriginalFormat
    public init(id: UUID = UUID(), name: String, dpi: Int = 300, duplex: Bool = false, format: OriginalFormat = .tiff) {
        self.id = id; self.name = name; self.dpi = dpi; self.duplex = duplex; self.format = format
    }
    public func validate() throws {
        guard !name.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty, name.utf8.count <= 128 else {
            throw ScannerError("invalidSettings", "Profile name must contain 1–128 bytes.")
        }
        try ScanRequest(dpi: dpi, duplex: duplex, format: format).validate()
    }
}
public struct JobSummary: Codable, Sendable {
    public var id: UUID
    public var scannerID: String
    public var profileID: UUID?
    public var batchID: UUID
    public var createdAt: Date
    public var startedAt: Date?
    public var completedAt: Date?
    public var state: ScanState
    public var progress: ScanProgress
    public var outputDirectory: String
    public var errors: [ScannerError]
    public init(_ record: ScanRecord) {
        id = record.id; scannerID = record.device.id; profileID = record.profile?.id; batchID = record.batchID
        createdAt = record.createdAt; startedAt = record.startedAt; completedAt = record.completedAt
        state = record.state; progress = record.progress; outputDirectory = record.outputDirectory; errors = record.errors
    }
}
public struct JobPage: Codable, Sendable {
    public var schemaVersion = 1
    public var jobs: [JobSummary]
    public var nextOffset: Int?
    public init(jobs: [JobSummary], nextOffset: Int?) { self.jobs = jobs; self.nextOffset = nextOffset }
}
public struct ServiceEvent: Codable, Sendable {
    public var cursor: Int64
    public var timestamp: Date
    public var kind: String
    public var payload: JSONValue
    public init(cursor: Int64, timestamp: Date, kind: String, payload: JSONValue) {
        self.cursor = cursor; self.timestamp = timestamp; self.kind = kind; self.payload = payload
    }
}
public struct EventPage: Codable, Sendable {
    public var schemaVersion = 1
    public var events: [ServiceEvent]
    public var nextCursor: Int64
    public var oldestCursor: Int64
    public var resetRequired: Bool
    public init(events: [ServiceEvent], after: Int64, oldestCursor: Int64, latestCursor: Int64) {
        self.events = events; self.nextCursor = events.last?.cursor ?? after; self.oldestCursor = oldestCursor
        resetRequired = after > latestCursor || (after > 0 && after < oldestCursor - 1)
    }
}
public struct ServiceStatus: Codable, Sendable {
    public var schemaVersion = 1
    public var activeJobID: UUID?
    public var queuedJobs: Int
    public var paused: Bool
    public var stopping: Bool
    public var persistenceError: ScannerError?
    public var storageRoot: String
    public init(activeJobID: UUID?, queuedJobs: Int, paused: Bool, stopping: Bool, persistenceError: ScannerError?, storageRoot: String) {
        self.activeJobID = activeJobID; self.queuedJobs = queuedJobs; self.paused = paused; self.stopping = stopping
        self.persistenceError = persistenceError; self.storageRoot = storageRoot
    }
}
