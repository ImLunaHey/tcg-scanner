import CryptoKit
import Darwin
import Foundation
import ImageIO
import ScannerAPI

extension ScannerCoordinator {
    public func imageMetadata(jobID: UUID, sequence: Int) async throws -> ImageMetadata {
        let record = try await scanStatus(jobID)
        guard let image = record.images.first(where: { $0.sequence == sequence }) else { throw ScannerError("imageNotFound", "No saved original with this sequence.") }
        return ImageMetadata(jobID: jobID, image: image)
    }
    public func thumbnail(jobID: UUID, sequence: Int) async throws -> ImageThumbnail {
        guard thumbnailWorkers < 2 else { throw ScannerError("resourceLimit", "At most two thumbnails may be generated concurrently.") }
        thumbnailWorkers += 1
        defer { thumbnailWorkers -= 1 }
        let record = try await scanStatus(jobID)
        guard let image = record.images.first(where: { $0.sequence == sequence }) else { throw ScannerError("imageNotFound", "Unknown saved image.") }
        guard image.bytes <= 32 * 1024 * 1024, image.width > 0, image.height > 0,
              image.width <= 20000, image.height <= 20000, image.width * image.height <= 100_000_000 else {
            throw ScannerError("resourceLimit", "Thumbnail source exceeds 32 MiB or 100 megapixels; download the original instead.")
        }
        let root = storageRoot
        return try await Task.detached(priority: .utility) {
            let chunk = try OriginalReader.read(root: root, record: record, image: image, offset: 0, length: image.bytes)
            guard chunk.chunkSHA256 == image.sha256, let data = Data(base64Encoded: chunk.base64),
                  let source = CGImageSourceCreateWithData(data as CFData, [kCGImageSourceShouldCache: false] as CFDictionary),
                  let preview = CGImageSourceCreateThumbnailAtIndex(source, 0, [
                    kCGImageSourceCreateThumbnailFromImageAlways: true,
                    kCGImageSourceCreateThumbnailWithTransform: true,
                    kCGImageSourceShouldCacheImmediately: false,
                    kCGImageSourceThumbnailMaxPixelSize: 512] as CFDictionary) else {
                throw ScannerError("storage", "Could not verify or decode thumbnail source.")
            }
            let result = NSMutableData()
            guard let destination = CGImageDestinationCreateWithData(result, "public.png" as CFString, 1, nil) else {
                throw ScannerError("storage", "Cannot encode thumbnail.")
            }
            CGImageDestinationAddImage(destination, preview, nil)
            guard CGImageDestinationFinalize(destination), result.length <= 2 * 1024 * 1024 else {
                throw ScannerError("storage", "Thumbnail encoding failed or exceeded size limit.")
            }
            let bytes = result as Data
            return ImageThumbnail(data: bytes, sha256: SHA256.hash(data: bytes).map { String(format: "%02x", $0) }.joined(),
                                  width: preview.width, height: preview.height)
        }.value
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
