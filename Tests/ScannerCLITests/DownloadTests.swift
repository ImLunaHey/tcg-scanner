import CryptoKit
import Foundation
import Testing
import ScannerAPI
@testable import ScannerCLI

@Test func downloadVerifiesChunksAndNeverOverwritesDestination() throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    defer { try? FileManager.default.removeItem(at: root) }
    let destination = root.appendingPathComponent("original.tiff")
    let bytes = Data("original driver bytes".utf8)
    func hash(_ data: Data) -> String { SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined() }
    let metadata = ImageMetadata(jobID: UUID(), image: OriginalImage(sequence: 1, relativePath: "originals/page-000001.tiff", bytes: bytes.count,
        sha256: hash(bytes), width: 1, height: 1, imageCount: 1, uniformType: "public.tiff"))
    try OriginalDownload.write(to: destination, metadata: metadata) { offset in
        let part = bytes.subdata(in: offset..<min(offset + 3, bytes.count))
        return ImageChunk(metadata: metadata, offset: offset, data: part, sha256: hash(part))
    }
    #expect(try Data(contentsOf: destination) == bytes)
    #expect(throws: ScannerError.self) {
        try OriginalDownload.write(to: destination, metadata: metadata) { _ in throw ScannerError("test", "Must not read") }
    }
    #expect(try Data(contentsOf: destination) == bytes)
    let invalid = root.appendingPathComponent("invalid.tiff")
    #expect(throws: ScannerError.self) {
        try OriginalDownload.write(to: invalid, metadata: metadata) { offset in
            ImageChunk(metadata: metadata, offset: offset, data: bytes, sha256: "bad hash")
        }
    }
    #expect(!FileManager.default.fileExists(atPath: invalid.path))
    #expect(try FileManager.default.contentsOfDirectory(atPath: root.path) == ["original.tiff"])
}
