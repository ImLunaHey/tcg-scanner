import Foundation
import ImageIO
import CoreGraphics
import Testing
import ScannerAPI
@testable import ScannerCore

func makeTestPNG(at url: URL) throws {
    let context = try #require(CGContext(data: nil, width: 8, height: 8, bitsPerComponent: 8, bytesPerRow: 32,
        space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue))
    let image = try #require(context.makeImage())
    let destination = try #require(CGImageDestinationCreateWithURL(url as CFURL, "public.png" as CFString, 1, nil))
    CGImageDestinationAddImage(destination, image, nil)
    #expect(CGImageDestinationFinalize(destination))
}
@Test func originalsAreBytePreservedOrderedAndNeverOverwritten() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let request = ScanRequest(duplex: true, format: .png)
    let store = try ScanStorage(root: root, device: Device(id: "mock", name: "Mock"), request: request)
    let a = store.incomingDirectory.appendingPathComponent("one.png")
    let b = store.incomingDirectory.appendingPathComponent("two.png")
    try makeTestPNG(at: a); try makeTestPNG(at: b)
    try await store.preserve(b, sequence: 2); try await store.preserve(a, sequence: 1)
    await #expect(throws: ScannerError.self) { try await store.preserve(a, sequence: 1) }
    try await store.finish(errors: [], cancelled: false, sessionClosed: true)
    let record = await store.snapshot()
    #expect(record.images.map(\.sequence) == [1, 2])
    #expect(record.state == .completed)
    #expect(try Data(contentsOf: a) == Data(contentsOf: URL(fileURLWithPath: record.outputDirectory).appendingPathComponent(record.images[0].relativePath)))
    #expect(throws: ScannerError.self) { try ScanStorage(root: root, device: record.device, request: request) }
}
@Test func storageRejectsEscapesAndPreservesInvalidImages() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let device = Device(id: "mock", name: "Mock")
    for path in ["../escape", "/etc", "a/../../escape"] {
        #expect(throws: ScannerError.self) { try ScanStorage(root: root, device: device, request: ScanRequest(outputDirectory: path)) }
    }
    try FileManager.default.createSymbolicLink(at: root.appendingPathComponent("link"), withDestinationURL: FileManager.default.temporaryDirectory)
    #expect(throws: ScannerError.self) { try ScanStorage(root: root, device: device, request: ScanRequest(outputDirectory: "link")) }
    let store = try ScanStorage(root: root, device: device, request: ScanRequest())
    let bad = store.incomingDirectory.appendingPathComponent("bad.tiff")
    try Data("invalid image".utf8).write(to: bad)
    await #expect(throws: ScannerError.self) { try await store.preserve(bad, sequence: 1) }
    #expect(FileManager.default.fileExists(atPath: bad.path))
    try await store.finish(errors: [ScannerError("test", "simulated failure")], cancelled: false, sessionClosed: true)
    #expect(await store.snapshot().state == .failed)
}

@Test func failedDuplexKeepsReceivedSideAndManifest() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let store = try ScanStorage(root: root, device: Device(id: "mock", name: "Mock"), request: ScanRequest(duplex: true, format: .png))
    let file = store.incomingDirectory.appendingPathComponent("partial.png")
    try makeTestPNG(at: file)
    try await store.preserve(file, sequence: 1)
    try await store.finish(errors: [ScannerError("disconnected", "Mock unplug")], cancelled: false, sessionClosed: false)
    let report = await store.snapshot()
    #expect(report.state == .failed)
    #expect(report.images.count == 1)
    #expect(report.warnings.contains(where: { $0.contains("Odd duplex") }))
    let persisted = try Wire.decoder().decode(ScanRecord.self, from: Data(contentsOf: URL(fileURLWithPath: report.outputDirectory).appendingPathComponent("manifest.json")))
    #expect(persisted.images[0].sha256 == report.images[0].sha256)
    #expect(FileManager.default.fileExists(atPath: file.path))
}
