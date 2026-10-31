import Foundation
import ScannerAPI
import ScannerImageProcessing

extension ScannerCoordinator {
    /// Inspect the exact saved source, including no-match backs. Preserve all
    /// identification and manual-review fields when committing the observations.
    public func checkCondition(jobID: UUID, sequence: Int, identificationID: UUID) async throws -> CardIdentification {
        guard !stopping, preparingCardJobs.isEmpty, !editingJobs.contains(jobID),
              identificationTasks.isEmpty, !automaticallyIdentifyingCards else {
            throw ScannerError("busy", "Another image operation is running. Retry shortly.")
        }
        preparingCardJobs.insert(jobID); defer { preparingCardJobs.remove(jobID) }
        try validateStorage()
        let snapshot = try await scanStatus(jobID)
        guard snapshot.state.terminal, scans[jobID] == nil,
              let identity = snapshot.cardIdentifications?.first(where: { $0.request.sequence == sequence && $0.request.id == identificationID }),
              identity.state != .processing else {
            throw ScannerError("conflict", "A completed, current card identification is required.")
        }
        let source: OriginalImage?
        if let editID = identity.sourceEditID { source = snapshot.processedImages?.first { $0.id == editID && $0.request.sequence == sequence }?.image }
        else { source = snapshot.images.first { $0.sequence == sequence } }
        guard let source, source.sha256 == identity.sourceSHA256, source.bytes <= 32 * 1024 * 1024 else {
            throw ScannerError("storage", "The identification's source image is missing or changed.")
        }
        let root = storageRoot
        let preparation = snapshot.processedImages?.first { $0.id == identity.sourceEditID }?.cardPreparation
        let task = Task.detached(priority: .utility) {
            try Task.checkCancellation()
            let chunk = try OriginalReader.read(root: root, record: snapshot, image: source, offset: 0, length: source.bytes)
            guard chunk.chunkSHA256 == source.sha256, let data = Data(base64Encoded: chunk.base64) else { throw ScannerError("storage", "Source checksum verification failed.") }
            let result = ConditionInspector.check(data, preparation: preparation)
            try Task.checkCancellation()
            return result
        }
        let estimate = try await withTaskCancellationHandler { try await task.value } onCancel: { task.cancel() }
        try validateStorage()
        var latest = try await scanStatus(jobID)
        guard let index = latest.cardIdentifications?.firstIndex(where: { $0.request.id == identificationID }),
              var current = latest.cardIdentifications?[index], current.state != .processing,
              current.sourceSHA256 == identity.sourceSHA256, current.sourceEditID == identity.sourceEditID else {
            throw ScannerError("conflict", "Identification changed during condition analysis. Refresh and retry.")
        }
        current.conditionCheck = estimate
        latest.cardIdentifications?[index] = current
        try database.save(latest); try BatchFiles.writeManifest(latest, root: storageRoot)
        return current
    }
}
