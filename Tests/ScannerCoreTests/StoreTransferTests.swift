import CoreGraphics
import CryptoKit
import Foundation
import ImageIO
import Testing
import ScannerAPI
import ScannerPersistence
import ScannerRecognition
@testable import ScannerCore

private struct OfflineStorePrices: CardPricingProviding {
    func prices(cardID: String) async throws -> CardMarketPrices { throw ScannerError("test", "Offline pricing fixture") }
    func sterlingRate() async throws -> EuroSterlingRate { throw ScannerError("test", "Offline pricing fixture") }
}

private actor NoStoreHardware: ScannerBackend {
    func devices() throws -> [Device] { throw ScannerError("test", "Store transfer must not access hardware.") }
    func inspect(deviceID: String) throws -> Inspection { throw ScannerError("test", "No hardware.") }
}
private actor StagingMock: StoreStaging {
    let origin = "https://store.example:5189"
    let scannerInstanceID = "fixture-scanner"
    var imported: StoreImport?
    var cards: [StoreCardReply] = []
    let batchID = UUID()
    var updateCalls = 0
    var uploadCalls = 0
    var loseUpdateResponse = false
    var raceUpdate = false
    var loseResponse = false
    var delay = false
    func setUpdateLoss() { loseUpdateResponse = true }
    func setUpdateRace() { raceUpdate = true }
    func markReviewed() { cards[0].notes = "Owner checked this card"; cards[0].revision += 1 }
    func setLoss() { loseResponse = true }
    func hold() { delay = true }
    func replacePhoto() { cards[0].front?.sha256 = "store-review-replacement" }
    func importBatch(_ value: StoreImport) async throws -> StoreImportReply {
        imported = value
        if delay { try await Task.sleep(for: .seconds(30)) }
        if cards.isEmpty {
            cards = value.cards.map { StoreCardReply(id: UUID(), scanBatchId: batchID, cardNumber: $0.cardNumber,
                revision: 1, status: "pending", front: nil, back: nil, review: $0.review, notes: "",
                game: $0.game, language: $0.language, gameSource: $0.gameSource, languageSource: $0.languageSource) }
        }
        return StoreImportReply(batch: StoreBatchReply(id: batchID, scannerInstanceId: value.scannerInstanceId,
            sourceBatchId: value.sourceBatchId, batchNumber: value.batchNumber, reviewUrl: "/admin/scan-inbox/\(batchID)"), cards: cards)
    }
    func update(card: StoreCardReply, input: StoreCardInput) throws -> StoreCardReply {
        updateCalls += 1
        let index = cards.firstIndex { $0.id == card.id }!
        if raceUpdate { raceUpdate = false; cards[index].notes = "Concurrent owner edit"; cards[index].revision += 1 }
        guard cards[index].revision == card.revision else { throw ScannerError("storeConflict", "Stale revision") }
        var result = card; result.revision += 1
        result.review = input.review; result.game = input.game; result.language = input.language
        result.gameSource = input.gameSource; result.languageSource = input.languageSource
        cards[index] = result
        if loseUpdateResponse { loseUpdateResponse = false; throw ScannerError("storeUnavailable", "Lost metadata response") }
        return result
    }
    func upload(card: StoreCardReply, front: Data, back: Data) throws -> StoreCardReply {
        uploadCalls += 1
        guard cards.first(where: { $0.id == card.id })?.revision == card.revision else {
            throw ScannerError("storeConflict", "Stale revision")
        }
        var result = card; result.revision += 1
        result.front = StorePhoto(id: UUID(), sha256: Self.hash(front), sizeBytes: front.count)
        result.back = StorePhoto(id: UUID(), sha256: Self.hash(back), sizeBytes: back.count)
        cards[cards.firstIndex { $0.id == card.id }!] = result
        if loseResponse { loseResponse = false; throw ScannerError("storeUnavailable", "Simulated lost response after durable upload.") }
        return result
    }
    static func hash(_ data: Data) -> String { SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined() }
}
private struct StoreFixture {
    let root: URL
    let db: ScannerDatabase
    var record: ScanRecord
    init() throws {
        root = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent(UUID().uuidString)
        let folder = root.appendingPathComponent("B-023")
        try FileManager.default.createDirectory(at: folder, withIntermediateDirectories: true)
        db = try ScannerDatabase()
        record = ScanRecord(device: Device(id: "fixture", name: "Fixture"), request: ScanRequest(duplex: true, prepareCards: true), outputDirectory: folder.path)
        record.batchNumber = 23; record.state = .completed
        record.processedImages = []; record.cardIdentifications = []
        for sequence in 1...2 {
            let context = try #require(CGContext(data: nil, width: 200, height: 280, bitsPerComponent: 8, bytesPerRow: 800,
                space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
            context.setFillColor(red: Double(sequence) / 3, green: 0.4, blue: 0.6, alpha: 1)
            context.fill(CGRect(x: 0, y: 0, width: 200, height: 280))
            let file = folder.appendingPathComponent(String(format: "C-%03d.png", sequence))
            let destination = try #require(CGImageDestinationCreateWithURL(file as CFURL, "public.png" as CFString, 1, nil))
            CGImageDestinationAddImage(destination, try #require(context.makeImage()), nil)
            #expect(CGImageDestinationFinalize(destination))
            let data = try Data(contentsOf: file)
            let original = OriginalImage(sequence: sequence, relativePath: file.lastPathComponent, bytes: data.count,
                sha256: StagingMock.hash(data), width: 200, height: 280, imageCount: 1, uniformType: "public.png")
            record.images.append(original)
            let editRequest = ImageEditRequest(jobID: record.id, sequence: sequence)
            var image = original; image.relativePath = "edit-\(editRequest.id.uuidString.lowercased()).png"
            try data.write(to: folder.appendingPathComponent(image.relativePath))
            var edit = ProcessedImage(request: editRequest, sourceSHA256: original.sha256, image: image)
            edit.cardPreparation = CardPreparation(crop: ImageCrop(x: 0, y: 0, width: 1, height: 1), quarterTurns: 0,
                orientationVerified: true, warnings: [], orientationBasis: sequence == 1 ? "pokemon-back-pattern" : "text")
            record.processedImages?.append(edit)
            var identity = CardIdentification(request: IdentificationRequest(jobID: record.id, sequence: sequence), sourceSHA256: image.sha256)
            identity.sourceEditID = edit.id; identity.state = sequence == 1 ? .noMatch : .reviewRequired
            identity.evidence = RecognitionEvidence(lines: [], candidates: sequence == 1 ? [] : [CardCandidate(id: "me04-047", name: "Claydol", localID: "047", setID: "me04", setName: "Chaos Rising", evidence: [])])
            record.cardIdentifications?.append(identity)
        }
        try db.insert(record, submission: record.request)
    }
    func coordinator(_ store: StagingMock?) throws -> ScannerCoordinator {
        try ScannerCoordinator(backend: NoStoreHardware(), storageRoot: root, database: db, cardPricing: OfflineStorePrices(), storeClient: store)
    }
}
private func waitForTransfer(_ coordinator: ScannerCoordinator, id: UUID) async throws -> StoreTransferStatus {
    for _ in 0..<300 {
        let status = try await coordinator.storeTransferStatus(id)
        if status.state != .sending { return status }
        try await Task.sleep(for: .milliseconds(10))
    }
    throw ScannerError("test", "Transfer did not finish.")
}

@Test func storeTransferPreservesLabelsPairsAndRetriesLostResponses() async throws {
    let fixture = try StoreFixture(); defer { try? FileManager.default.removeItem(at: fixture.root) }
    let store = StagingMock(); await store.setLoss()
    let coordinator = try fixture.coordinator(store)
    #expect(try await coordinator.sendToStore(fixture.record.id).state == .sending)
    #expect(try await coordinator.sendToStore(fixture.record.id).state == .sending)
    #expect(try await waitForTransfer(coordinator, id: fixture.record.id).state == .failed)
    #expect(await store.uploadCalls == 1)
    let payload = try #require(await store.imported)
    #expect(payload.batchNumber == "B-023" && payload.cards[0].cardNumber == "C-001")
    #expect(payload.sourceBatchId == fixture.record.id && payload.cards[0].review.recognitionSuggestions[0].name == "Claydol")
    #expect(payload.cards[0].review.conditionEstimate == nil)
    #expect(payload.cards[0].game == "pokemon" && payload.cards[0].language == "en")
    #expect(payload.cards[0].gameSource == "recognition" && payload.cards[0].languageSource == "scannerConfiguration")
    #expect(payload.cards[0].review.recognitionSuggestions[0].provider == "tcgdex")
    #expect(payload.cards[0].review.recognitionSuggestions[0].externalId == "me04-047")
    let remote = try #require(await store.cards.first)
    #expect(remote.front?.sha256 == fixture.record.images[1].sha256)
    #expect(remote.back?.sha256 == fixture.record.images[0].sha256)
    _ = try await coordinator.sendToStore(fixture.record.id)
    #expect(try await waitForTransfer(coordinator, id: fixture.record.id).state == .sent)
    #expect(await store.uploadCalls == 1) // The lost response never duplicates the uploaded pair.
    #expect(try await coordinator.sendToStore(fixture.record.id).state == .sending)
    #expect(try await waitForTransfer(coordinator, id: fixture.record.id).state == .sent)
    #expect(await store.uploadCalls == 1)
    #expect(await store.updateCalls == 0)
    #expect(try fixture.db.job(fixture.record.id)?.images.map(\.sha256) == fixture.record.images.map(\.sha256))
    await coordinator.shutdownScans()
    #expect(try await fixture.coordinator(store).storeTransferStatus(fixture.record.id).state == .sent)
}

@Test func storeTransferRejectsUncertainSidesAndRecoversInterruptedLedger() async throws {
    let fixture = try StoreFixture(); defer { try? FileManager.default.removeItem(at: fixture.root) }
    let store = StagingMock()
    var ambiguous = fixture.record
    ambiguous.processedImages?[0].cardPreparation?.orientationBasis = "unknown"
    ambiguous.cardIdentifications = []
    #expect(throws: ScannerError.self) { try StoreTransferLedger.make(record: ambiguous, store: store, valuation: nil) }
    var ledger = try StoreTransferLedger.make(record: fixture.record, store: store, valuation: nil)
    ledger.status.state = .sending
    try fixture.db.saveStoreTransfer(ledger.status, record: Wire.encoder().encode(ledger))
    let coordinator = try fixture.coordinator(store)
    #expect(try await coordinator.storeTransferStatus(fixture.record.id).state == .failed)
    _ = try await coordinator.sendToStore(fixture.record.id)
    #expect(try await waitForTransfer(coordinator, id: fixture.record.id).state == .sent)
    #expect(await store.uploadCalls == 1)
    await coordinator.shutdownScans()
}

@Test func storeTransferShutdownAndConfigurationDoNotAffectScanningHistory() async throws {
    let fixture = try StoreFixture(); defer { try? FileManager.default.removeItem(at: fixture.root) }
    let disabled = try fixture.coordinator(nil)
    #expect(try await disabled.storeTransferStatus(fixture.record.id).configured == false)
    await #expect(throws: ScannerError.self) { try await disabled.sendToStore(fixture.record.id) }
    let store = StagingMock(); await store.hold()
    let coordinator = try fixture.coordinator(store)
    _ = try await coordinator.sendToStore(fixture.record.id)
    await coordinator.shutdownScans()
    #expect(try await coordinator.storeTransferStatus(fixture.record.id).state == .failed)
    #expect(try fixture.db.job(fixture.record.id)?.state == .completed)
}

@Test func storeClientUsesOnlyStagingEndpointsAndRejectsHostileLinks() async throws {
    let client = StoreClient(origin: "https://store.example", scannerInstanceID: "fixture", token: "private-fixture-token") { request in
        #expect(request.url?.path == "/api/v1/admin/scan-batches")
        #expect(request.value(forHTTPHeaderField: "Authorization") == "Bearer private-fixture-token")
        #expect(request.httpMethod == "POST")
        return (Data("private-fixture-token".utf8), 403)
    }
    do {
        _ = try await client.importBatch(StoreImport(scannerInstanceId: "fixture", sourceBatchId: UUID(), batchNumber: "B-023", cards: []))
        Issue.record("Expected authentication failure")
    } catch {
        #expect(!String(describing: error).contains("private-fixture-token"))
    }
    for path in ["https://evil.example/", "//evil.example/", "javascript:alert(1)", "https://user:pass@store.example/"] {
        #expect(throws: ScannerError.self) { try ScannerCoordinator.storeReviewURL(path, origin: "https://store.example") }
    }
    #expect(try ScannerCoordinator.storeReviewURL("/admin/scan-inbox", origin: "https://store.example") == "https://store.example/admin/scan-inbox")
}

@Test func retryNeverReplacesStoreReviewPhotos() async throws {
    let fixture = try StoreFixture(); defer { try? FileManager.default.removeItem(at: fixture.root) }
    let store = StagingMock(); await store.setLoss()
    let coordinator = try fixture.coordinator(store)
    _ = try await coordinator.sendToStore(fixture.record.id)
    #expect(try await waitForTransfer(coordinator, id: fixture.record.id).state == .failed)
    await store.replacePhoto()
    _ = try await coordinator.sendToStore(fixture.record.id)
    let result = try await waitForTransfer(coordinator, id: fixture.record.id)
    #expect(result.state == .sent && result.preservedCards == ["C-001"])
    #expect(result.sentCards == 0)
    #expect(await store.uploadCalls == 1)
    await coordinator.shutdownScans()
}

@Test func storeConfigurationRejectsPublicCredentialsAndUnsafeOrigins() throws {
    let root = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let token = root.appendingPathComponent("token")
    try Data(String(repeating: "x", count: 64).utf8).write(to: token)
    try FileManager.default.setAttributes([.posixPermissions: 0o644], ofItemAtPath: token.path)
    var config = StoreConfiguration(baseURL: "https://store.example", scannerInstanceID: "test", tokenFile: token.path)
    #expect(throws: ScannerError.self) { try config.token() }
    try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: token.path)
    #expect(try config.token().count == 64)
    for origin in ["http://store.example", "https://user:pass@store.example", "https://store.example/path", "https://store.example/?token=x"] {
        config.baseURL = origin
        #expect(throws: ScannerError.self) { try config.validatedURL() }
    }
}

@Test func storeReportsOrientationFailureSeparatelyFromPNGSize() throws {
    let fixture = try StoreFixture(); defer { try? FileManager.default.removeItem(at: fixture.root) }
    var record = fixture.record
    record.processedImages?[1].cardPreparation?.orientationVerified = false
    do {
        _ = try StoreTransferLedger.make(record: record, store: StagingMock(), valuation: nil)
        Issue.record("Expected unverified orientation to block export")
    } catch let error as ScannerError {
        #expect(error.message.contains("capture 2") && error.message.contains("uncertain orientation"))
        #expect(!error.message.contains("size limits"))
    }
}

@Test func storeConditionNotesContainOnlyDistinctObservedMarks() throws {
    let fixture = try StoreFixture(); defer { try? FileManager.default.removeItem(at: fixture.root) }
    var record = fixture.record
    let warning = "Card is close to a scan boundary. Check that the original captured the entire edge; cropping cannot restore missing pixels."
    for index in 0...1 { record.processedImages?[index].cardPreparation?.warnings = [warning] }
    record.cardIdentifications?[0].conditionCheck = ConditionCheck(state: .checked, limitations: ["Not a grade"])
    let empty = try StoreTransferLedger.make(record: record, store: StagingMock(), valuation: nil)
    #expect(empty.submission.cards[0].review.borderFindings.isEmpty)
    #expect(empty.submission.cards[0].review.conditionEstimate == nil)
    #expect(empty.snapshot.processedImages?[0].cardPreparation?.warnings == [warning])

    let mark = ConditionCheck.Finding(kind: .edgeWhitening,
        region: ImageCrop(x: 0, y: 0.2, width: 0.02, height: 0.1), explanation: "Possible whitening along the left edge.")
    record.cardIdentifications?[0].conditionCheck?.findings = [mark, mark]
    let observed = try StoreTransferLedger.make(record: record, store: StagingMock(), valuation: nil)
    #expect(observed.submission.cards[0].review.borderFindings == ["Back: Possible whitening along the left edge."])
    record.cardIdentifications?[0].conditionCheck?.state = .inconclusive
    let inconclusive = try StoreTransferLedger.make(record: record, store: StagingMock(), valuation: nil)
    #expect(inconclusive.submission.cards[0].review.borderFindings.isEmpty)
}


private func changeStoreSource(_ fixture: StoreFixture) throws {
    var record = fixture.record
    // A different valid PNG simulates a new prepared front without altering originals.
    let data = try Data(contentsOf: URL(fileURLWithPath: record.outputDirectory).appendingPathComponent(record.images[0].relativePath))
    var edit = try #require(record.processedImages?.last)
    edit.request = ImageEditRequest(jobID: record.id, sequence: 2)
    edit.image.relativePath = "edit-\(edit.request.id.uuidString.lowercased()).png"
    edit.image.sha256 = StagingMock.hash(data); edit.image.bytes = data.count
    try data.write(to: URL(fileURLWithPath: record.outputDirectory).appendingPathComponent(edit.image.relativePath))
    record.processedImages?.append(edit)
    record.cardIdentifications?[1].sourceSHA256 = edit.image.sha256
    record.cardIdentifications?[1].sourceEditID = edit.id
    record.cardIdentifications?[1].evidence?.candidates[0].name = "Corrected match"
    record.cardIdentifications?[1].finish = .normal
    try fixture.db.save(record)
}

@Test func storeResyncUsesLatestImagesAndMetadataWithoutDuplicatingCards() async throws {
    let fixture = try StoreFixture(); defer { try? FileManager.default.removeItem(at: fixture.root) }
    let store = StagingMock(); let coordinator = try fixture.coordinator(store)
    _ = try await coordinator.sendToStore(fixture.record.id)
    #expect(try await waitForTransfer(coordinator, id: fixture.record.id).state == .sent)
    let originalRemote = try #require(await store.cards.first)
    try changeStoreSource(fixture)
    await store.setUpdateLoss()
    _ = try await coordinator.sendToStore(fixture.record.id)
    #expect(try await waitForTransfer(coordinator, id: fixture.record.id).state == .failed)
    #expect(await store.uploadCalls == 1)
    // Retry the durable target after a lost metadata response, including a service restart.
    await coordinator.shutdownScans()
    let restarted = try fixture.coordinator(store)
    _ = try await restarted.sendToStore(fixture.record.id)
    let status = try await waitForTransfer(restarted, id: fixture.record.id)
    #expect(status.state == .sent && status.sentCards == 1 && status.preservedCards == [])
    let remote = try #require(await store.cards.first)
    #expect(remote.id == originalRemote.id && remote.scanBatchId == originalRemote.scanBatchId)
    #expect(remote.review?.recognitionSuggestions.first?.name == "Corrected match")
    #expect(remote.review?.finishEstimate == "normal")
    #expect(remote.front?.sha256 != originalRemote.front?.sha256)
    #expect(remote.back?.sha256 == originalRemote.back?.sha256)
    #expect(await store.cards.count == 1)
    #expect(await store.updateCalls == 1)
    #expect(await store.uploadCalls == 2)
    _ = try await restarted.sendToStore(fixture.record.id)
    #expect(try await waitForTransfer(restarted, id: fixture.record.id).state == .sent)
    #expect(await store.updateCalls == 1)
    #expect(await store.uploadCalls == 2)
    await restarted.shutdownScans()
}

@Test func storeResyncPreservesReviewerChangesAndRevisionRaces() async throws {
    let fixture = try StoreFixture(); defer { try? FileManager.default.removeItem(at: fixture.root) }
    let store = StagingMock(); let coordinator = try fixture.coordinator(store)
    _ = try await coordinator.sendToStore(fixture.record.id)
    _ = try await waitForTransfer(coordinator, id: fixture.record.id)
    try changeStoreSource(fixture)
    await store.setUpdateRace()
    _ = try await coordinator.sendToStore(fixture.record.id)
    let raced = try await waitForTransfer(coordinator, id: fixture.record.id)
    #expect(raced.state == .failed && raced.error?.kind == "storeConflict")
    #expect(await store.uploadCalls == 1)
    _ = try await coordinator.sendToStore(fixture.record.id)
    let retried = try await waitForTransfer(coordinator, id: fixture.record.id)
    #expect(retried.state == .sent && retried.preservedCards == ["C-001"] && retried.sentCards == 0)
    #expect(await store.cards.first?.notes == "Concurrent owner edit")
    #expect(await store.cards.first?.review?.recognitionSuggestions.first?.name == "Claydol")
    // Another explicit resync still preserves the reviewer, rather than adopting their changes.
    _ = try await coordinator.sendToStore(fixture.record.id)
    #expect(try await waitForTransfer(coordinator, id: fixture.record.id).preservedCards == ["C-001"])
    #expect(await store.uploadCalls == 1)
    await coordinator.shutdownScans()
}

@Test func storeResyncPreservesConfirmedRejectedManualAndChangedValues() throws {
    let fixture = try StoreFixture(); defer { try? FileManager.default.removeItem(at: fixture.root) }
    let ledger = try StoreTransferLedger.make(record: fixture.record, store: StagingMock(), valuation: nil)
    let input = ledger.submission.cards[0], card = ledger.cards[0]
    let baseline = StoreSyncBaseline(input: input, card: card)
    let remote = StoreCardReply(id: UUID(), scanBatchId: UUID(), cardNumber: "C-001", revision: 2,
        status: "pending", front: nil, back: nil, review: input.review, notes: "",
        game: input.game, language: input.language, gameSource: input.gameSource, languageSource: input.languageSource)
    #expect(ScannerCoordinator.canSync(remote, input: input, card: card, baseline: baseline))
    var reviewed = remote; reviewed.confirmed = .object([:])
    #expect(!ScannerCoordinator.canSync(reviewed, input: input, card: card, baseline: baseline))
    for state in ["approved", "rejected"] {
        reviewed = remote; reviewed.status = state
        #expect(!ScannerCoordinator.canSync(reviewed, input: input, card: card, baseline: baseline))
    }
    reviewed = remote; reviewed.languageSource = "manual"
    #expect(!ScannerCoordinator.canSync(reviewed, input: input, card: card, baseline: baseline))
    reviewed = remote; reviewed.review?.suggestedPriceGbp = 123
    #expect(!ScannerCoordinator.canSync(reviewed, input: input, card: card, baseline: baseline))
    reviewed = remote; reviewed.review = nil
    #expect(!ScannerCoordinator.canSync(reviewed, input: input, card: card, baseline: baseline))
}


@Test func storeClientUpdatesOnlyRevisionCheckedDraftFields() async throws {
    let fixture = try StoreFixture(); defer { try? FileManager.default.removeItem(at: fixture.root) }
    let plan = try StoreTransferLedger.make(record: fixture.record, store: StagingMock(), valuation: nil)
    let input = plan.submission.cards[0]
    let remote = StoreCardReply(id: UUID(), scanBatchId: UUID(), cardNumber: "C-001", revision: 7,
        status: "pending", front: nil, back: nil, review: input.review, notes: "",
        game: input.game, language: input.language, gameSource: input.gameSource, languageSource: input.languageSource)
    let client = StoreClient(origin: "https://store.example", scannerInstanceID: "fixture", token: "fixture") { request in
        #expect(request.httpMethod == "PUT")
        #expect(request.url?.path == "/api/v1/admin/scan-cards/\(remote.id.uuidString)")
        let value = try Wire.decoder().decode(JSONValue.self, from: #require(request.httpBody))
        guard case .object(let fields) = value else { throw ScannerError("test", "Expected object") }
        #expect(fields["revision"] == .number(7))
        #expect(fields["confirmed"] == .null)
        #expect(fields["status"] == .string("pending"))
        #expect(fields["game"] == .string("pokemon") && fields["language"] == .string("en"))
        #expect(fields["cardNumber"] == nil && fields["id"] == nil)
        return (try Wire.encoder().encode(remote), 200)
    }
    #expect(try await client.update(card: remote, input: input).id == remote.id)
}

@Test func yugiohPairsCannotBeExportedAsPokemon() throws {
    let fixture = try StoreFixture(); defer { try? FileManager.default.removeItem(at: fixture.root) }
    var record = fixture.record
    record.processedImages?[0].cardPreparation?.orientationBasis = "yugioh-back-pattern"
    #expect(record.detectedGame(for: 1) == .yugioh)
    #expect(throws: ScannerError.self) { try StoreTransferLedger.make(record: record, store: StagingMock(), valuation: nil) }
}
