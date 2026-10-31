import Foundation
import ScannerAPI

extension ScannerCoordinator {
    public func jobs(limit: Int = 50, offset: Int = 0) throws -> JobPage { try database.jobs(limit: limit, offset: offset) }
    public func profiles() throws -> [ScanProfile] { try database.profiles() }
    public func saveProfile(_ profile: ScanProfile) throws {
        guard !stopping else { throw ScannerError("busy", "Service is shutting down.") }
        try database.saveProfile(profile)
    }
    public func deleteProfile(_ id: UUID) throws { try database.deleteProfile(id) }
    public func status() -> ServiceStatus {
        ServiceStatus(activeJobID: activeScanID, queuedJobs: queue.count, paused: paused || requiresRestart, stopping: stopping,
                      persistenceError: persistenceError, storageRoot: storageRoot.path)
    }
    public func events(after: Int64, limit: Int = 100, waitSeconds: Int = 0) async throws -> EventPage {
        guard (0...20).contains(waitSeconds) else { throw ScannerError("invalidParams", "Event wait must be 0–20 seconds.") }
        guard eventWaiters < 8 else { throw ScannerError("resourceLimit", "At most eight event subscriptions may wait concurrently.") }
        eventWaiters += 1
        defer { eventWaiters -= 1 }
        let deadline = ContinuousClock.now + .seconds(waitSeconds)
        while true {
            let page = try database.events(after: after, limit: limit)
            if !page.events.isEmpty || page.resetRequired || stopping || ContinuousClock.now >= deadline { return page }
            try await Task.sleep(for: .milliseconds(200))
        }
    }
}
