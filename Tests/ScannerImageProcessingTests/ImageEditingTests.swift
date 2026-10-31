import Foundation
import CoreGraphics
import ImageIO
import Testing
import ScannerAPI
import ScannerCore
import ScannerPersistence
import ScannerImageProcessing

private func fixture() throws -> Data {
    let context = try #require(CGContext(data: nil, width: 120, height: 80, bitsPerComponent: 8, bytesPerRow: 480, space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
    context.setFillColor(CGColor(red: 1, green: 0, blue: 0, alpha: 1)); context.fill(CGRect(x: 0, y: 0, width: 120, height: 80))
    let data = NSMutableData()
    let destination = try #require(CGImageDestinationCreateWithData(data, "public.png" as CFString, 1, nil))
    CGImageDestinationAddImage(destination, try #require(context.makeImage()), nil)
    #expect(CGImageDestinationFinalize(destination))
    return data as Data
}
private actor Paper: ScannerBackend {
    func devices() -> [Device] { [Device(id: "mock", name: "Paper")] }
    func inspect(deviceID: String) -> Inspection { Inspection(device: Device(id: deviceID, name: "Paper")) }
    func capture(deviceID: String, request: ScanRequest, store: ScanStorage) async throws -> CaptureOutcome {
        try await store.markScanning(settings: [:])
        let url = store.incomingDirectory.appendingPathComponent("page.png")
        try fixture().write(to: url)
        try await store.preserve(url, sequence: 1)
        return CaptureOutcome()
    }
}
@Test func cropAndClockwiseRotationDimensionsAndInputValidation() throws {
    let bytes = try fixture()
    var request = ImageEditRequest(jobID: UUID(), sequence: 1, crop: ImageCrop(x: 0.25, y: 0.25, width: 0.5, height: 0.5), quarterTurns: 1)
    let result = try ImageProcessor.render(bytes, request: request)
    #expect(result.width == 40 && result.height == 60)
    #expect(result.data.starts(with: [137, 80, 78, 71]))
    request.crop.x = .infinity
    #expect(throws: ScannerError.self) { try ImageProcessor.render(bytes, request: request) }
    request.crop = ImageCrop(x: 0.9, width: 0.5)
    #expect(throws: ScannerError.self) { try request.validate() }
}
@Test func editsAreIdempotentAndPreserveOriginalsAndRejectSymlinks() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let database = try ScannerDatabase()
    let coordinator = try ScannerCoordinator(backend: Paper(), storageRoot: root, database: database)
    var record = try await coordinator.startScan(ScanRequest())
    let deadline = ContinuousClock.now + .seconds(5)
    while !record.state.terminal && ContinuousClock.now < deadline {
        try await Task.sleep(for: .milliseconds(10)); record = try await coordinator.scanStatus(record.id)
    }
    #expect(record.state == .completed)
    let path = URL(fileURLWithPath: record.outputDirectory).appendingPathComponent(try #require(record.images.first).relativePath)
    let original = try Data(contentsOf: path)
    let request = ImageEditRequest(jobID: record.id, sequence: 1, quarterTurns: 1)
    let first = try await coordinator.editImage(request)
    let second = try await coordinator.editImage(request)
    #expect(first.image.sha256 == second.image.sha256)
    #expect(first.createdAt == second.createdAt)
    // Simulate a crash after files were committed but before the ledger recorded the edit.
    try database.save(record)
    let restarted = try ScannerCoordinator(backend: Paper(), storageRoot: root, database: database)
    let recovered = try await restarted.editImage(request)
    #expect(recovered.id == first.id && recovered.createdAt == first.createdAt)
    #expect(try await restarted.scanStatus(record.id).processedImages?.count == 1)
    var changed = request; changed.quarterTurns = 2
    await #expect(throws: ScannerError.self) { try await coordinator.editImage(changed) }
    #expect(try Data(contentsOf: path) == original)
    let thumbnail = try await coordinator.thumbnail(jobID: record.id, sequence: 1, editID: first.id)
    #expect(thumbnail.width == 80 && thumbnail.height == 120)
    let saved = try await coordinator.scanStatus(record.id)
    #expect(saved.processedImages?.count == 1)
    let manifest = try Wire.decoder().decode(ScanRecord.self, from: Data(contentsOf: URL(fileURLWithPath: record.outputDirectory).appendingPathComponent("manifest.json")))
    #expect(manifest.processedImages?.first?.id == first.id)
    let processed = URL(fileURLWithPath: record.outputDirectory).appendingPathComponent(first.image.relativePath)
    try FileManager.default.removeItem(at: processed)
    try FileManager.default.createSymbolicLink(at: processed, withDestinationURL: path)
    await #expect(throws: ScannerError.self) { try await coordinator.readImage(jobID: record.id, sequence: 1, offset: 0, length: 100, editID: first.id) }
}
