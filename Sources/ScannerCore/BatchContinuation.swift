import Foundation
import ScannerAPI

extension ScannerCoordinator {
    public func continueBatch(_ id: UUID, requestID: UUID) async throws -> ScanRecord {
        if let existing = try existingContinuation(id, requestID: requestID) { return existing }
        try acceptingContinuation(id)
        guard let previous = try database.job(id) else { throw ScannerError("scanNotFound", "Unknown batch.") }
        _ = try await selectDevice(previous.device.id)
        // Device discovery suspends; another client may have accepted this feed.
        if let existing = try existingContinuation(id, requestID: requestID) { return existing }
        try acceptingContinuation(id)
        guard let current = try database.job(id) else { throw ScannerError("scanNotFound", "Unknown batch.") }
        let database = self.database
        let store = try ScanStorage(root: storageRoot, continuing: current, requestID: requestID,
            validateStorage: validateStorage, persist: database.save)
        let record = store.initialRecord
        try database.beginContinuation(record, requestID: requestID)
        scans[id] = store; queue.append(id)
        scheduleNext()
        return record
    }
    private func existingContinuation(_ id: UUID, requestID: UUID) throws -> ScanRecord? {
        if let batch = try database.continuationBatch(requestID) {
            guard batch == id else { throw ScannerError("conflict", "Continuation request ID belongs to another batch.") }
            return try database.job(id)
        }
        guard try database.submission(requestID) == nil else { throw ScannerError("conflict", "Request ID belongs to a scan submission.") }
        return nil
    }
    private func acceptingContinuation(_ id: UUID) throws {
        try acceptingJobs()
        guard scans[id] == nil, preparingCardJobs.isEmpty, editingJobs.isEmpty,
              identificationTasks.isEmpty, !automaticallyIdentifyingCards,
              !preparingStoreTransfer, storeTransferTask == nil else {
            throw ScannerError("busy", "Wait for scanning, image analysis and Store transfers before continuing a batch.")
        }
    }
}
