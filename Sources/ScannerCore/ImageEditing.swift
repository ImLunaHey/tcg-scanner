import CryptoKit
import Darwin
import Foundation
import ScannerAPI
import ScannerImageProcessing

extension ScannerCoordinator {
    public func editImage(_ request: ImageEditRequest) async throws -> ProcessedImage {
        try request.validate()
        try validateStorage()
        guard !stopping, editingJobs.count < 2, !editingJobs.contains(request.jobID) else { throw ScannerError("busy", "Another image edit is running; retry shortly.") }
        editingJobs.insert(request.jobID)
        defer { editingJobs.remove(request.jobID) }
        var record = try await scanStatus(request.jobID)
        if let existing = record.processedImages?.first(where: { $0.id == request.id }) {
            guard existing.request == request else { throw ScannerError("conflict", "Edit ID already has different inputs.") }
            try EditFiles.writeManifest(record, root: storageRoot)
            return existing
        }
        guard record.state.terminal, scans[record.id] == nil else { throw ScannerError("busy", "Wait for the scan to finish before editing.") }
        guard (record.processedImages ?? []).filter({ $0.request.sequence == request.sequence }).count < 8 else { throw ScannerError("resourceLimit", "At most eight saved edits per original are supported.") }
        guard let original = record.images.first(where: { $0.sequence == request.sequence }) else { throw ScannerError("imageNotFound", "Original not found.") }
        guard original.bytes <= 32 * 1024 * 1024 else { throw ScannerError("resourceLimit", "Editing source exceeds 32 MiB.") }
        let root = storageRoot
        let snapshot = record
        let edit = try await Task.detached(priority: .utility) {
            if let existing = try EditFiles.recover(request, original: original, root: root, record: snapshot) { return existing }
            let source = try OriginalReader.read(root: root, record: snapshot, image: original, offset: 0, length: original.bytes)
            guard source.chunkSHA256 == original.sha256, let bytes = Data(base64Encoded: source.base64) else { throw ScannerError("storage", "Original checksum verification failed.") }
            let output = try ImageProcessor.render(bytes, request: request)
            let image = OriginalImage(sequence: request.sequence, relativePath: "edit-\(request.id.uuidString.lowercased()).png", bytes: output.data.count,
                sha256: SHA256.hash(data: output.data).map { String(format: "%02x", $0) }.joined(), width: output.width, height: output.height, imageCount: 1, uniformType: "public.png")
            let edit = ProcessedImage(request: request, sourceSHA256: original.sha256, image: image)
            try EditFiles.writeImage(output.data, edit: edit, root: root, record: snapshot)
            return edit
        }.value
        record.processedImages = (record.processedImages ?? []) + [edit]
        // The actor serializes manifest/ledger publication. Originals are never overwritten.
        try database.save(record)
        try EditFiles.writeManifest(record, root: root)
        return edit
    }
}

private enum EditFiles {
    /// Complete sidecars allow a retry after a crash between file publication and ledger commit.
    static func recover(_ request: ImageEditRequest, original: OriginalImage, root: URL, record: ScanRecord) throws -> ProcessedImage? {
        let batch = try directory(root: root, record: record); defer { Darwin.close(batch) }
        let fd = batch
        guard let metadata = try read(at: fd, name: ".edit-" + request.id.uuidString.lowercased() + ".json", limit: 65536) else { return nil }
        let edit = try Wire.decoder().decode(ProcessedImage.self, from: metadata)
        guard edit.request == request, edit.sourceSHA256 == original.sha256 else { throw ScannerError("conflict", "Retained edit has different inputs.") }
        guard let image = try read(at: fd, name: "edit-" + request.id.uuidString.lowercased() + ".png", limit: 64 * 1024 * 1024),
              image.count == edit.image.bytes,
              SHA256.hash(data: image).map({ String(format: "%02x", $0) }).joined() == edit.image.sha256 else { throw ScannerError("storage", "Retained edit failed integrity checks.") }
        return edit
    }
    static func read(at fd: Int32, name: String, limit: Int) throws -> Data? {
        let file = openat(fd, name, O_RDONLY | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC)
        if file < 0 && errno == ENOENT { return nil }
        guard file >= 0 else { throw ScannerError("accessDenied", "Unsafe edit file.") }
        defer { Darwin.close(file) }
        var info = stat()
        guard fstat(file, &info) == 0, info.st_mode & S_IFMT == S_IFREG, info.st_nlink == 1, info.st_size > 0, info.st_size <= limit else { throw ScannerError("storage", "Invalid edit file size or type.") }
        var data = Data(count: Int(info.st_size)); var offset = 0
        while offset < data.count {
            let n = data.withUnsafeMutableBytes { Darwin.read(file, $0.baseAddress!.advanced(by: offset), $0.count - offset) }
            if n < 0 && errno == EINTR { continue }
            guard n > 0 else { throw ScannerError("storage", "Incomplete edit file.") }
            offset += n
        }
        return data
    }
    static func directory(root: URL, record: ScanRecord) throws -> Int32 {
        let root = root.standardizedFileURL.resolvingSymlinksInPath()
        let path = URL(fileURLWithPath: record.outputDirectory).standardizedFileURL
        guard path.path.hasPrefix(root.path + "/") else { throw ScannerError("accessDenied", "Batch is outside configured storage.") }
        var fd = open(root.path, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)
        guard fd >= 0 else { throw ScannerError("storage", "Cannot open scan storage.") }
        for part in path.pathComponents.dropFirst(root.pathComponents.count) {
            let next = openat(fd, part, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC)
            Darwin.close(fd)
            guard next >= 0 else { throw ScannerError("accessDenied", "Unsafe batch directory.") }
            fd = next
        }
        return fd
    }
    static func write(_ data: Data, at fd: Int32, name: String) throws {
        let file = openat(fd, name, O_WRONLY | O_CREAT | O_EXCL | O_NOFOLLOW | O_CLOEXEC, 0o600)
        guard file >= 0 else { throw ScannerError("storage", "Output already exists or cannot be created; retained files were not overwritten.") }
        defer { Darwin.close(file) }
        try data.withUnsafeBytes { buffer in
            var offset = 0
            while offset < buffer.count {
                let n = Darwin.write(file, buffer.baseAddress!.advanced(by: offset), buffer.count - offset)
                if n < 0 && errno == EINTR { continue }
                guard n > 0 else { throw ScannerError("storage", "Cannot write edited output; partial file retained.") }
                offset += n
            }
        }
        guard fsync(file) == 0 else { throw ScannerError("storage", "Cannot synchronize edited output.") }
    }
    static func writeImage(_ data: Data, edit: ProcessedImage, root: URL, record: ScanRecord) throws {
        let batch = try directory(root: root, record: record); defer { Darwin.close(batch) }
        let fd = batch
        try write(data, at: fd, name: "edit-" + edit.id.uuidString.lowercased() + ".png")
        try write(Wire.encoder().encode(edit), at: fd, name: ".edit-" + edit.id.uuidString.lowercased() + ".json")
        _ = fsync(fd)
    }
    static func writeManifest(_ record: ScanRecord, root: URL) throws {
        let fd = try directory(root: root, record: record); defer { Darwin.close(fd) }
        let temp = ".manifest-" + UUID().uuidString
        try write(Wire.encoder().encode(record), at: fd, name: temp)
        guard renameat(fd, temp, fd, "manifest.json") == 0, fsync(fd) == 0 else { throw ScannerError("storage", "Cannot publish edited manifest; originals retained.") }
    }
}
