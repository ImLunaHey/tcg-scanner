import CryptoKit
import Darwin
import Foundation
import ScannerAPI

extension ScannerCoordinator {
    public func imageMetadata(jobID: UUID, sequence: Int) async throws -> ImageMetadata {
        let record = try await scanStatus(jobID)
        guard let image = record.images.first(where: { $0.sequence == sequence }) else { throw ScannerError("imageNotFound", "No saved original with this sequence.") }
        return ImageMetadata(jobID: jobID, image: image)
    }
    public func readImage(jobID: UUID, sequence: Int, offset: Int, length: Int) async throws -> ImageChunk {
        guard (0...512 * 1024 * 1024).contains(offset), (1...262_144).contains(length) else {
            throw ScannerError("invalidParams", "Image reads require offset 0–512 MiB and length 1–262144.")
        }
        let record = try await scanStatus(jobID)
        guard let image = record.images.first(where: { $0.sequence == sequence }) else { throw ScannerError("imageNotFound", "Unknown saved image.") }
        let root = storageRoot
        return try await Task.detached(priority: .utility) {
            try OriginalReader.read(root: root, record: record, image: image, offset: offset, length: length)
        }.value
    }
}

/// Opens each component relative to a pinned directory descriptor, rejecting symlinks.
/// The caller supplies only job ID and image sequence, never a filesystem path.
private enum OriginalReader {
    static func read(root: URL, record: ScanRecord, image: OriginalImage, offset: Int, length: Int) throws -> ImageChunk {
        let root = root.standardizedFileURL.resolvingSymlinksInPath()
        let directory = URL(fileURLWithPath: record.outputDirectory).standardizedFileURL
        let expected = String(format: "originals/page-%06d.%@", image.sequence, record.request.format.rawValue)
        guard directory.path.hasPrefix(root.path + "/"), image.relativePath == expected, offset <= image.bytes else {
            throw ScannerError("accessDenied", "Image is outside configured storage or has invalid metadata/offset.")
        }
        let components = Array(directory.pathComponents.dropFirst(root.pathComponents.count)) + expected.split(separator: "/").map(String.init)
        guard components.allSatisfy({ !$0.isEmpty && $0 != "." && $0 != ".." }) else { throw ScannerError("accessDenied", "Invalid image path.") }
        var fd = open(root.path, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)
        guard fd >= 0 else { throw ScannerError("storage", "Cannot open configured scan storage.") }
        defer { Darwin.close(fd) }
        for (index, component) in components.enumerated() {
            let flags = O_RDONLY | O_NOFOLLOW | O_CLOEXEC | O_NONBLOCK | (index == components.count - 1 ? 0 : O_DIRECTORY)
            let next = openat(fd, component, flags)
            guard next >= 0 else { throw ScannerError("storage", "Saved image is missing or an unsafe path component was encountered.") }
            Darwin.close(fd); fd = next
        }
        var info = stat()
        guard fstat(fd, &info) == 0, info.st_mode & S_IFMT == S_IFREG, info.st_nlink == 1,
              info.st_size == image.bytes, info.st_size <= 512 * 1024 * 1024 else {
            throw ScannerError("storage", "Saved original is not a regular file with the recorded size.")
        }
        let count = min(length, image.bytes - offset)
        var bytes = [UInt8](repeating: 0, count: count)
        var consumed = 0
        while consumed < count {
            let read = bytes.withUnsafeMutableBytes { buffer in
                pread(fd, buffer.baseAddress!.advanced(by: consumed), count - consumed, off_t(offset + consumed))
            }
            if read < 0 && errno == EINTR { continue }
            guard read > 0 else { throw ScannerError("storage", "Could not read the recorded original bytes.") }
            consumed += read
        }
        let data = Data(bytes)
        return ImageChunk(metadata: ImageMetadata(jobID: record.id, image: image), offset: offset, data: data,
                          sha256: SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined())
    }
}
