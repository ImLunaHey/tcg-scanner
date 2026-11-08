import CryptoKit
import Foundation
import ScannerAPI
import ScannerImageProcessing

extension ScannerCoordinator {
    public func prepareCard(jobID: UUID, sequence: Int, id: UUID) async throws -> ProcessedImage {
        guard !stopping, preparingCardJobs.isEmpty, !editingJobs.contains(jobID) else { throw ScannerError("busy", "Another image operation is running. Retry shortly.") }
        guard ScanLimits.imageSequences.contains(sequence) else { throw ScannerError("invalidParams", "Sequence must be 1–1000.") }
        preparingCardJobs.insert(jobID); defer { preparingCardJobs.remove(jobID) }
        try validateStorage()
        var record = try await scanStatus(jobID)
        guard record.state.terminal, scans[jobID] == nil else { throw ScannerError("busy", "Wait for scanning to finish.") }
        if let existing = record.processedImages?.first(where: { $0.id == id }) {
            guard existing.request.sequence == sequence, existing.cardPreparation != nil else { throw ScannerError("conflict", "Image operation ID already has different inputs.") }
            if record.processedImages?.last(where: { $0.request.sequence == sequence && $0.cardPreparation != nil })?.id == existing.id {
                record.warnings.removeAll { $0.hasPrefix("Card preparation failed for image \(sequence):") }
                try database.save(record)
            }
            try BatchFiles.writeManifest(record, root: storageRoot)
            return existing
        }
        guard let image = record.images.first(where: { $0.sequence == sequence }) else { throw ScannerError("imageNotFound", "Original image not found.") }
        guard (record.processedImages ?? []).filter({ $0.request.sequence == sequence }).count < 8 else { throw ScannerError("resourceLimit", "At most eight saved copies per original are supported.") }
        let edit = try await renderPreparedCard(record: record, image: image, id: id)
        // Reload after analysis: a concurrent identification or edit may have been saved.
        var updated = try await scanStatus(jobID)
        updated.processedImages = (updated.processedImages ?? []) + [edit]
        updated.warnings.removeAll { $0.hasPrefix("Card preparation failed for image \(sequence):") }
        updated.cardGames = CardGameDetection.resolve(updated)
        try database.save(updated); try BatchFiles.writeManifest(updated, root: storageRoot)
        return edit
    }

    func prepareCapturedCards(_ store: ScanStorage) async {
        // Manual requests stay excluded while the bounded automatic pool runs.
        automaticallyIdentifyingCards = true
        defer { automaticallyIdentifyingCards = false }
        for worker in Array(identificationTasks.values) { await worker.value }
        let snapshot = await store.snapshot()
        let pairs = Dictionary(grouping: snapshot.images.filter { $0.sequence > store.startingImageCount }, by: { ($0.sequence + 1) / 2 })
            .sorted { $0.key < $1.key }.map { $0.value.sorted { $0.sequence < $1.sequence } }
        await CardProcessingPool.run(pairs) { images in
            await self.prepareCapturedPair(images, snapshot: snapshot, store: store)
        }
    }

    private func prepareCapturedPair(_ images: [OriginalImage], snapshot: ScanRecord, store: ScanStorage) async {
        for image in images {
            guard !Task.isCancelled else { return }
            do {
                let edit = try await renderPreparedCard(record: snapshot, image: image, id: UUID())
                try await store.addPreparedImage(edit)
            } catch is CancellationError { return }
            catch {
                try? await store.addPreparationWarning("Card preparation failed for image \(image.sequence): \(error.localizedDescription). Original retained; use manual review.")
            }
        }
        // Both sides must be prepared before game/side routing. Each pair can
        // finish independently; cards are still displayed in acquisition order.
        let current = await store.snapshot()
        for image in images {
            guard !Task.isCancelled else { return }
            let sequence = image.sequence
            let game = current.detectedGame(for: sequence)
            let pair = CardGameDetection.resolve(current).first { $0.number == (sequence + 1) / 2 }
            guard game == .starwarsUnlimited || game == .pokemon || (game == .yugioh && pair?.frontSequence == sequence) else { continue }
            do {
                let prepared = current.processedImages?.last { $0.request.sequence == sequence && $0.cardPreparation != nil }
                let source = prepared?.image ?? image
                var pending = CardIdentification(request: IdentificationRequest(jobID: current.id, sequence: sequence,
                    language: current.request.resolvedCardLanguage.rawValue), sourceSHA256: source.sha256)
                pending.sourceEditID = prepared?.id
                if game == .yugioh { pending.method = "apple-vision-ygoprodeck-text-v1" }
                if game == .starwarsUnlimited { pending.method = "apple-vision-swu-db-text-v1" }
                try await store.saveIdentification(pending)
                let result = await recognizeCard(pending, record: current, image: source)
                try await store.saveIdentification(result)
            } catch {
                try? await store.addPreparationWarning("Identification could not be saved for image \(sequence): \(error.localizedDescription). Original retained; retry from review.")
            }
        }
    }

    private func renderPreparedCard(record: ScanRecord, image: OriginalImage, id: UUID) async throws -> ProcessedImage {
        guard image.bytes <= 32 * 1024 * 1024 else { throw ScannerError("resourceLimit", "Card source exceeds 32 MiB.") }
        let root = storageRoot
        let validate = validateStorage
        let task = Task.detached(priority: .utility) {
            try Task.checkCancellation(); try validate()
            let original = try OriginalReader.read(root: root, record: record, image: image, offset: 0, length: image.bytes)
            guard original.chunkSHA256 == image.sha256, let data = Data(base64Encoded: original.base64) else { throw ScannerError("storage", "Original checksum verification failed.") }
            let plan = try CardProcessor.analyze(data, background: record.request.background ?? .black, language: record.request.resolvedCardLanguage)
            let request = ImageEditRequest(id: id, jobID: record.id, sequence: image.sequence, crop: plan.crop, quarterTurns: plan.quarterTurns, deskewDegrees: plan.deskewDegrees)
            if var recovered = try BatchFiles.recover(request, original: image, root: root, record: record) {
                recovered.cardPreparation = plan; return recovered
            }
            let result = try ImageProcessor.render(data, request: request)
            try Task.checkCancellation(); try validate()
            let prepared = OriginalImage(sequence: image.sequence, relativePath: "edit-\(id.uuidString.lowercased()).png", bytes: result.data.count,
                sha256: SHA256.hash(data: result.data).map { String(format: "%02x", $0) }.joined(), width: result.width, height: result.height, imageCount: 1, uniformType: "public.png")
            var edit = ProcessedImage(request: request, sourceSHA256: image.sha256, image: prepared)
            edit.cardPreparation = plan
            try BatchFiles.writeImage(result.data, edit: edit, root: root, record: record)
            return edit
        }
        return try await withTaskCancellationHandler { try await task.value } onCancel: { task.cancel() }
    }
}
