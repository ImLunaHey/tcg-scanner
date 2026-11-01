import Foundation
import ScannerAPI
import ScannerRecognition
import ScannerImageProcessing

extension ScannerCoordinator {
    public func identifyCard(_ request: IdentificationRequest) async throws -> CardIdentification {
        try request.validate()
        guard !stopping else { throw ScannerError("busy", "Service is shutting down.") }
        try validateStorage()
        var record = try await scanStatus(request.jobID)
        guard record.state.terminal, scans[record.id] == nil else { throw ScannerError("busy", "Wait until the scan finishes before identifying images.") }
        guard let original = record.images.first(where: { $0.sequence == request.sequence }) else { throw ScannerError("imageNotFound", "Original image not found.") }
        if let existing = record.cardIdentifications?.first(where: { $0.request.id == request.id }) {
            guard existing.request == request else { throw ScannerError("conflict", "Identification ID already has different inputs.") }
            try BatchFiles.writeManifest(record, root: storageRoot)
            return existing
        }
        if let existing = record.cardIdentifications?.first(where: { $0.request.sequence == request.sequence }),
           ![IdentificationState.failed, .noMatch, .rejected].contains(existing.state) {
            throw ScannerError("conflict", "This image already has an identification. Review or reject it before trying again.")
        }
        guard identificationTasks.isEmpty, !automaticallyIdentifyingCards else { throw ScannerError("busy", "Another identification is running; retry when it finishes.") }
        let prepared = record.processedImages?.last(where: { $0.request.sequence == request.sequence && $0.cardPreparation != nil })
        let image = prepared?.image ?? original
        guard image.bytes <= 32 * 1024 * 1024 else { throw ScannerError("resourceLimit", "OCR source exceeds 32 MiB.") }
        var identification = CardIdentification(request: request, sourceSHA256: image.sha256)
        identification.sourceEditID = prepared?.id
        record.cardIdentifications = (record.cardIdentifications ?? []).filter { $0.request.sequence != request.sequence } + [identification]
        try database.save(record)
        do { try BatchFiles.writeManifest(record, root: storageRoot) }
        catch {
            var failed = identification; failed.state = .failed; failed.completedAt = Date(); failed.error = ScannerError("storage", error: error)
            replaceIdentification(failed, in: &record); try database.save(record)
            throw error
        }
        identificationTasks[request.id] = Task { await runIdentification(identification, record: record, image: image) }
        return identification
    }

    public func cardIdentification(jobID: UUID, sequence: Int) async throws -> CardIdentification? {
        guard (1...100).contains(sequence) else { throw ScannerError("invalidParams", "Sequence must be 1–100.") }
        let record = try await scanStatus(jobID)
        guard record.images.contains(where: { $0.sequence == sequence }) else { throw ScannerError("imageNotFound", "Original image not found.") }
        return record.cardIdentifications?.first { $0.request.sequence == sequence }
    }

    public func reviewCard(jobID: UUID, sequence: Int, identificationID: UUID, cardID: String?, finish: CardFinish, condition: CardCondition) async throws -> CardIdentification {
        guard !stopping else { throw ScannerError("busy", "Service is shutting down.") }
        try validateStorage()
        var record = try await scanStatus(jobID)
        guard record.state.terminal, scans[jobID] == nil else { throw ScannerError("busy", "Wait for card processing to finish before reviewing.") }
        guard var result = record.cardIdentifications?.first(where: { $0.request.sequence == sequence && $0.request.id == identificationID }) else {
            throw ScannerError("conflict", "Identification changed or is missing. Refresh before reviewing.")
        }
        guard result.state != .processing else { throw ScannerError("busy", "Identification is still running.") }
        if let cardID {
            guard result.evidence?.candidates.contains(where: { $0.id == cardID }) == true else {
                throw ScannerError("invalidParams", "Select one of this identification's candidate IDs, or reject the suggestions.")
            }
            result.state = .confirmed; result.selectedCardID = cardID
            result.finish = finish; result.condition = condition
        } else {
            result.state = .rejected; result.selectedCardID = nil; result.finish = .unverified; result.condition = .unassessed
        }
        result.reviewedAt = Date()
        replaceIdentification(result, in: &record)
        try database.save(record)
        try BatchFiles.writeManifest(record, root: storageRoot)
        return result
    }

    private func runIdentification(_ pending: CardIdentification, record snapshot: ScanRecord, image: OriginalImage) async {
        defer { identificationTasks[pending.request.id] = nil }
        let result = await recognizeCard(pending, record: snapshot, image: image)
        do {
            guard var record = try database.job(pending.request.jobID) else { return }
            replaceIdentification(result, in: &record)
            try database.save(record)
            do { try validateStorage(); try BatchFiles.writeManifest(record, root: storageRoot) }
            catch {
                record.warnings.append("Identification is saved locally, but the on-disk manifest could not be updated. Retry the same identification request after restoring storage.")
                try database.save(record)
            }
        } catch { persistenceError = ScannerError("storage", error: error) }
    }

    // Both the scan pipeline and explicit retries use the same bounded recognizer.
    func recognizeCard(_ pending: CardIdentification, record snapshot: ScanRecord, image: OriginalImage) async -> CardIdentification {
        var result = pending
        do {
            guard image.bytes <= 32 * 1024 * 1024 else { throw ScannerError("resourceLimit", "OCR source exceeds 32 MiB.") }
            try validateStorage()
            let root = storageRoot
            let sourceTask = Task.detached(priority: .utility) {
                try Task.checkCancellation()
                let source = try OriginalReader.read(root: root, record: snapshot, image: image, offset: 0, length: image.bytes)
                guard source.chunkSHA256 == image.sha256, let data = Data(base64Encoded: source.base64) else {
                    throw ScannerError("storage", "Original checksum verification failed.")
                }
                return data
            }
            let data = try await withTaskCancellationHandler { try await sourceTask.value } onCancel: { sourceTask.cancel() }
            try Task.checkCancellation()
            let preparation = snapshot.processedImages?.first { $0.id == pending.sourceEditID }?.cardPreparation
            let check = Task.detached(priority: .utility) { ConditionInspector.check(data, preparation: preparation) }
            result.conditionCheck = await withTaskCancellationHandler { await check.value } onCancel: { check.cancel() }
            try Task.checkCancellation()
            result.evidence = try await cardRecognizer.recognize(data)
            if let evidence = result.evidence, evidence.candidates.count == 1 {
                let estimate = Task.detached(priority: .utility) { FinishEstimator.estimate(data, evidence: evidence, preparation: preparation) }
                result.finishEstimate = await withTaskCancellationHandler { await estimate.value } onCancel: { estimate.cancel() }
                try Task.checkCancellation()
            }
            try Task.checkCancellation()
            result.state = result.evidence?.candidates.isEmpty == false ? .reviewRequired : .noMatch
        } catch {
            result.state = .failed
            result.error = error is CancellationError ? ScannerError("interrupted", "Identification stopped; retry explicitly.") : ScannerError("recognition", error: error)
        }
        result.completedAt = Date()
        return result
    }
    private func replaceIdentification(_ result: CardIdentification, in record: inout ScanRecord) {
        record.cardIdentifications = (record.cardIdentifications ?? []).filter { $0.request.sequence != result.request.sequence } + [result]
    }
}
