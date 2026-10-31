import CoreGraphics
import Foundation
import ImageIO
import Testing
import ScannerAPI
import ScannerCore
import ScannerPersistence

private actor CardPaper: ScannerBackend {
    let blank: Bool
    init(blank: Bool = false) { self.blank = blank }
    func devices() -> [Device] { [Device(id: "mock", name: "Mock card source")] }
    func inspect(deviceID: String) -> Inspection { Inspection(device: devices()[0]) }
    func capture(deviceID: String, request: ScanRequest, store: ScanStorage) async throws -> CaptureOutcome {
        try await store.markScanning(settings: [:])
        let context = try #require(CGContext(data: nil, width: 400, height: 600, bitsPerComponent: 8, bytesPerRow: 1600,
            space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
        context.setFillColor(red: 0, green: 0, blue: 0, alpha: 1); context.fill(CGRect(x: 0, y: 0, width: 400, height: 600))
        if !blank { context.setFillColor(red: 0.8, green: 0.7, blue: 0.5, alpha: 1); context.fill(CGRect(x: 100, y: 250, width: 180, height: 250)) }
        let file = store.incomingDirectory.appendingPathComponent("mock.png")
        let destination = try #require(CGImageDestinationCreateWithURL(file as CFURL, "public.png" as CFString, 1, nil))
        CGImageDestinationAddImage(destination, try #require(context.makeImage()), nil)
        #expect(CGImageDestinationFinalize(destination))
        try await store.preserve(file, sequence: 1)
        return CaptureOutcome()
    }
}
private func waitForCard(_ coordinator: ScannerCoordinator, id: UUID) async throws -> ScanRecord {
    let deadline = ContinuousClock.now + .seconds(10)
    while ContinuousClock.now < deadline {
        let record = try await coordinator.scanStatus(id)
        if record.state.terminal { return record }
        try await Task.sleep(for: .milliseconds(10))
    }
    throw ScannerError("test", "Card processing timeout.")
}
@Test func optionalAutomaticCardPreparationPreservesOriginalAndManifest() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let database = try ScannerDatabase()
    let coordinator = try ScannerCoordinator(backend: CardPaper(), storageRoot: root, database: database)
    let start = try await coordinator.startScan(ScanRequest(prepareCards: true))
    let record = try await waitForCard(coordinator, id: start.id)
    #expect(record.state == .completed && record.images.count == 1)
    let prepared = try #require(record.processedImages?.first)
    #expect(prepared.cardPreparation != nil && prepared.image.width < 220 && prepared.image.height < 290)
    let original = try Data(contentsOf: URL(fileURLWithPath: record.outputDirectory).appendingPathComponent("C-001.png"))
    let id = UUID()
    let first = try await coordinator.prepareCard(jobID: record.id, sequence: 1, id: id)
    let repeated = try await coordinator.prepareCard(jobID: record.id, sequence: 1, id: id)
    #expect(first.image.sha256 == repeated.image.sha256)
    #expect(try await coordinator.scanStatus(record.id).processedImages?.count == 2)
    try database.save(record) // Simulate interruption between copy publication and ledger commit.
    let recovered = try await coordinator.prepareCard(jobID: record.id, sequence: 1, id: id)
    #expect(recovered.image.sha256 == first.image.sha256)
    #expect(try Data(contentsOf: URL(fileURLWithPath: record.outputDirectory).appendingPathComponent("C-001.png")) == original)
    let manifest = try Wire.decoder().decode(ScanRecord.self, from: Data(contentsOf: URL(fileURLWithPath: record.outputDirectory).appendingPathComponent("manifest.json")))
    #expect(manifest.processedImages?.count == 2)
    await coordinator.shutdownScans()
}
@Test func failedCardDetectionDoesNotLoseOrFailOriginalScan() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let coordinator = try ScannerCoordinator(backend: CardPaper(blank: true), storageRoot: root)
    let job = try await coordinator.startScan(ScanRequest(prepareCards: true))
    let record = try await waitForCard(coordinator, id: job.id)
    #expect(record.state == .completed && record.images.count == 1)
    #expect(record.processedImages == nil)
    #expect(record.warnings.contains { $0.contains("Card preparation failed") })
    await coordinator.shutdownScans()
}
