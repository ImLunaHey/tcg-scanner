import CryptoKit
import Foundation
import ScannerAPI

extension ScannerCoordinator {
    public func storeTransferStatus(_ id: UUID) async throws -> StoreTransferStatus {
        _ = try await scanStatus(id)
        guard let data = try database.storeTransfer(id) else { return StoreTransferStatus(jobID: id, configured: storeClient != nil) }
        var status = try Wire.decoder().decode(StoreTransferLedger.self, from: data).status
        status.configured = storeClient != nil
        return status
    }
    public func sendToStore(_ id: UUID) async throws -> StoreTransferStatus {
        guard !stopping, !paused else { throw ScannerError("busy", "Service is stopping or paused.") }
        guard let client = storeClient else { throw ScannerError("storeNotConfigured", "Store integration is not configured on the scanner service.") }
        if let data = try database.storeTransfer(id) {
            let saved = try Wire.decoder().decode(StoreTransferLedger.self, from: data)
            if [.sent, .sending].contains(saved.status.state) { return saved.status }
        }
        guard !preparingStoreTransfer, storeTransferTask == nil else { throw ScannerError("busy", "Another batch is being sent to Store.") }
        preparingStoreTransfer = true; defer { preparingStoreTransfer = false }
        var ledger: StoreTransferLedger
        if let data = try database.storeTransfer(id) { ledger = try Wire.decoder().decode(StoreTransferLedger.self, from: data) }
        else {
            let record = try await scanStatus(id)
            let valuation = try await batchValuation(id)
            ledger = try StoreTransferLedger.make(record: record, store: client, valuation: valuation)
        }
        // Upgrade only a frozen local transfer snapshot, never Store reviewer data.
        // Older persisted receipts remain readable; all new requests use the current contract.
        if ledger.submission.cards.contains(where: { $0.game == nil || $0.language == nil }) {
            let refreshed = try StoreTransferLedger.make(record: ledger.snapshot, store: client, valuation: nil)
            for index in ledger.submission.cards.indices {
                var updated = refreshed.submission.cards[index]
                updated.review.suggestedPriceGbp = ledger.submission.cards[index].review.suggestedPriceGbp
                ledger.submission.cards[index] = updated
            }
        }
        guard ledger.origin == client.origin, ledger.submission.scannerInstanceId == client.scannerInstanceID else {
            throw ScannerError("storeConflict", "This transfer belongs to a different Store configuration. Restore the original connection before retrying.")
        }
        guard !stopping, !paused else { throw ScannerError("busy", "Service paused while preparing the transfer.") }
        ledger.status.state = .sending; ledger.status.sentCards = 0; ledger.status.error = nil
        try saveStoreLedger(&ledger)
        let snapshot = ledger
        storeTransferTask = Task { await transferToStore(snapshot, client: client) }
        return ledger.status
    }
    private func saveStoreLedger(_ ledger: inout StoreTransferLedger) throws {
        ledger.status.updatedAt = Date()
        try database.saveStoreTransfer(ledger.status, record: Wire.encoder().encode(ledger))
    }
    private func transferToStore(_ snapshot: StoreTransferLedger, client: any StoreStaging) async {
        var ledger = snapshot
        defer { storeTransferTask = nil }
        do {
            try Task.checkCancellation()
            let imported = try await client.importBatch(ledger.submission)
            guard imported.batch.scannerInstanceId == ledger.submission.scannerInstanceId,
                  imported.batch.sourceBatchId == ledger.snapshot.id, imported.batch.batchNumber == ledger.submission.batchNumber,
                  Set(imported.cards.map(\.cardNumber)) == Set(ledger.submission.cards.map(\.cardNumber)),
                  imported.cards.count == ledger.cards.count, Set(imported.cards.map(\.id)).count == ledger.cards.count,
                  imported.cards.allSatisfy({ $0.scanBatchId == imported.batch.id }) else {
                throw ScannerError("storeResponse", "Store returned mismatched batch/card identities.")
            }
            ledger.status.reviewURL = try Self.storeReviewURL(imported.batch.reviewUrl, origin: client.origin)
            try saveStoreLedger(&ledger)
            for card in ledger.cards {
                try Task.checkCancellation()
                let label = "C-" + BatchNaming.number(card.number)
                guard let remote = imported.cards.first(where: { $0.cardNumber == label }) else { throw ScannerError("storeResponse", "Store omitted a staged card.") }
                if Self.photosMatch(remote, card: card) {
                    // A retry after a lost response or approval preserves Store changes.
                } else {
                    guard remote.front == nil, remote.back == nil, remote.status == "pending" else {
                        throw ScannerError("storeConflict", "\(label) already has different photos or has been reviewed in Store. Existing review data was preserved.")
                    }
                    let record = ledger.snapshot, root = storageRoot, validate = validateStorage
                    let read = Task.detached(priority: .utility) {
                        try Task.checkCancellation(); try validate()
                        func bytes(_ image: OriginalImage) throws -> Data {
                            let result = try OriginalReader.read(root: root, record: record, image: image, offset: 0, length: image.bytes)
                            guard result.chunkSHA256 == image.sha256, let data = Data(base64Encoded: result.base64),
                                  data.starts(with: [137, 80, 78, 71, 13, 10, 26, 10]) else {
                                throw ScannerError("integrity", "Prepared PNG checksum verification failed. Nothing was overwritten.")
                            }
                            return data
                        }
                        return try (bytes(card.front), bytes(card.back))
                    }
                    let (front, back) = try await withTaskCancellationHandler { try await read.value } onCancel: { read.cancel() }
                    try Task.checkCancellation()
                    let uploaded = try await client.upload(card: remote, front: front, back: back)
                    guard uploaded.id == remote.id, uploaded.scanBatchId == remote.scanBatchId, uploaded.cardNumber == label,
                          Self.photosMatch(uploaded, card: card) else {
                        throw ScannerError("storeResponse", "Store did not confirm the expected paired photo checksums.")
                    }
                }
                ledger.status.sentCards += 1; try saveStoreLedger(&ledger)
            }
            ledger.status.state = .sent; try saveStoreLedger(&ledger)
        } catch {
            ledger.status.state = .failed
            ledger.status.error = (error as? ScannerError) ?? ScannerError("storeInterrupted", "Transfer stopped. Retry sending; existing Store cards are retained.")
            do { try saveStoreLedger(&ledger) } catch { persistenceError = ScannerError("storage", "Could not save Store transfer status.") }
        }
    }
    static func photosMatch(_ remote: StoreCardReply, card: StorePlannedCard) -> Bool {
        remote.front?.sha256 == card.front.sha256 && remote.back?.sha256 == card.back.sha256
            && remote.front?.sizeBytes == card.front.bytes && remote.back?.sizeBytes == card.back.bytes
    }
    static func storeReviewURL(_ path: String, origin: String) throws -> String {
        guard let base = URL(string: origin), let url = URL(string: path, relativeTo: base)?.absoluteURL,
              url.scheme == "https", url.host == base.host, url.port == base.port,
              url.user == nil, url.password == nil else { throw ScannerError("storeResponse", "Store returned an unsafe review link.") }
        return url.absoluteString
    }
}
