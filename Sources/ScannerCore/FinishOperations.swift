import Foundation
import ScannerAPI
import ScannerRecognition

extension ScannerCoordinator {
    /// Re-evaluate a saved identification without rejecting it, feeding hardware,
    /// downloading a model, or replacing a confirmed finish/condition.
    public func estimateFinish(jobID: UUID, sequence: Int, identificationID: UUID) async throws -> CardIdentification {
        guard !stopping, preparingCardJobs.isEmpty, !editingJobs.contains(jobID),
              identificationTasks.isEmpty, !automaticallyIdentifyingCards else {
            throw ScannerError("busy", "Another image operation is running. Retry shortly.")
        }
        preparingCardJobs.insert(jobID); defer { preparingCardJobs.remove(jobID) }
        try validateStorage()
        let snapshot = try await scanStatus(jobID)
        guard snapshot.state.terminal, scans[jobID] == nil,
              let identity = snapshot.cardIdentifications?.first(where: { $0.request.sequence == sequence && $0.request.id == identificationID }),
              [.reviewRequired, .confirmed, .noMatch].contains(identity.state), var evidence = identity.evidence else {
            throw ScannerError("conflict", "A completed, current card identification is required.")
        }
        guard let language = CardLanguage(rawValue: identity.request.language), language == snapshot.request.resolvedCardLanguage else {
            throw ScannerError("conflict", "Identification language does not match the batch's configured card language.")
        }
        if let selected = identity.selectedCardID { evidence.candidates = evidence.candidates.filter { $0.id == selected } }
        let source: OriginalImage?
        let prepared = snapshot.processedImages?.first { $0.id == identity.sourceEditID && $0.request.sequence == sequence }
        if identity.sourceEditID != nil { source = prepared?.image }
        else { source = snapshot.images.first { $0.sequence == sequence } }
        guard let source, source.sha256 == identity.sourceSHA256, source.bytes <= 32 * 1024 * 1024 else {
            throw ScannerError("storage", "The identification's source image is missing or changed.")
        }
        let root = storageRoot, input = evidence, analyzer = finishAnalyzer
        let task = Task.detached(priority: .utility) {
            try Task.checkCancellation()
            let chunk = try OriginalReader.read(root: root, record: snapshot, image: source, offset: 0, length: source.bytes)
            guard chunk.chunkSHA256 == source.sha256, let data = Data(base64Encoded: chunk.base64) else { throw ScannerError("storage", "Source checksum verification failed.") }
            let result = await analyzer.estimate(data, evidence: input, preparation: prepared?.cardPreparation, language: language)
            try Task.checkCancellation()
            return result
        }
        let estimate = try await withTaskCancellationHandler { try await task.value } onCancel: { task.cancel() }
        try validateStorage()
        var latest = try await scanStatus(jobID)
        guard let index = latest.cardIdentifications?.firstIndex(where: { $0.request.id == identificationID }),
              var current = latest.cardIdentifications?[index], [.reviewRequired, .confirmed, .noMatch].contains(current.state),
              current.sourceSHA256 == identity.sourceSHA256, current.selectedCardID == identity.selectedCardID else {
            throw ScannerError("conflict", "Identification changed during finish analysis. Refresh and retry.")
        }
        current.finishEstimate = estimate
        latest.cardIdentifications?[index] = current
        try database.save(latest); try BatchFiles.writeManifest(latest, root: storageRoot)
        return current
    }
}
