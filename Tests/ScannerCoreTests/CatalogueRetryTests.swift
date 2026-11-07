import Foundation
import Testing
import ScannerAPI
import ScannerPersistence

@Test func catalogueRetryDeadlinesSurviveRestartAndExcludeReviewedCards() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let path = root.appendingPathComponent("state.sqlite").path
    let database = try ScannerDatabase(path: path)
    var record = ScanRecord(device: Device(id: "mock", name: "Mock"), request: ScanRequest(), outputDirectory: root.path)
    record.state = .completed
    var pending = CardIdentification(request: IdentificationRequest(jobID: record.id, sequence: 1), sourceSHA256: "source")
    pending.state = .awaitingCatalogue
    pending.catalogueRetryAfter = Date(timeIntervalSince1970: 2_000_000_000)
    pending.evidence = RecognitionEvidence(lines: [], candidates: [])
    pending.evidence?.uncataloguedCard = UncataloguedCard(name: "Kyogre", setCode: "DLR", collectorNumber: "026/103")
    record.cardIdentifications = [pending]
    try database.insert(record, submission: record.request)
    let reopened = try ScannerDatabase(path: path)
    let restored = try #require(try reopened.nextCatalogueRetryJob())
    #expect(restored.cardIdentifications?.first?.catalogueRetryAfter == pending.catalogueRetryAfter)
    #expect(restored.cardIdentifications?.first?.evidence?.uncataloguedCard?.name == "Kyogre")
    for state in [IdentificationState.confirmed, .rejected, .processing, .reviewRequired, .noMatch] {
        record.cardIdentifications?[0].state = state
        try database.save(record)
        #expect(try reopened.nextCatalogueRetryJob() == nil)
    }
    record.cardIdentifications?[0].state = .failed
    try database.save(record)
    #expect(try reopened.nextCatalogueRetryJob()?.id == record.id)
}
