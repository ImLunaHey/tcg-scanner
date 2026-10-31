import CryptoKit
import Darwin
import Foundation
import ImageIO
import ScannerAPI

/// Owns one unique batch. Driver output and partial files are retained on failure.
public actor ScanStorage {
    public nonisolated let incomingDirectory: URL
    public nonisolated let initialRecord: ScanRecord
    private let directory: URL
    private var record: ScanRecord
    private let validateStorage: @Sendable () throws -> Void
    private let persist: @Sendable (ScanRecord) throws -> Void

    public init(root: URL, device: Device, request: ScanRequest, batchNumber: Int? = nil, queued: Bool = false, profile: ScanProfile? = nil,
                validateStorage: @escaping @Sendable () throws -> Void = {},
                persist: @escaping @Sendable (ScanRecord) throws -> Void = { _ in }) throws {
        self.persist = persist; self.validateStorage = validateStorage
        try validateStorage()
        try request.validate()
        let root = root.standardizedFileURL.resolvingSymlinksInPath()
        let proposed = request.outputDirectory.map { value in
            value.hasPrefix("/") ? URL(fileURLWithPath: value) : root.appendingPathComponent(value)
        } ?? root
        guard !proposed.pathComponents.contains("..") else { throw ScannerError("storage", "Parent traversal is not allowed.") }
        let parent = proposed.standardizedFileURL
        guard parent.path == root.path || parent.path.hasPrefix(root.path + "/"),
              parent.resolvingSymlinksInPath().path == parent.path else {
            throw ScannerError("storage", "Output must be inside the configured storage root without symlinks: \(root.path)")
        }
        try FileManager.default.createDirectory(at: parent, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        guard parent.resolvingSymlinksInPath().path == parent.path else { throw ScannerError("storage", "Output path changed during creation.") }
        guard batchNumber == nil || batchNumber! > 0 else { throw ScannerError("invalidSettings", "Batch number must be positive.") }
        let createdAt = Date()
        directory = parent.appendingPathComponent(batchNumber.map(BatchNaming.batchFolder) ?? "batch-" + request.requestID.uuidString.lowercased(), isDirectory: true)
        guard mkdir(directory.path, 0o700) == 0 else {
            throw ScannerError("storage", "Cannot create unique batch directory; existing batches are never overwritten (\(String(cString: strerror(errno)))).")
        }
        incomingDirectory = directory.appendingPathComponent(".incoming", isDirectory: true)
        try FileManager.default.createDirectory(at: incomingDirectory, withIntermediateDirectories: false, attributes: [.posixPermissions: 0o700])
        record = ScanRecord(device: device, request: request, outputDirectory: directory.path)
        record.createdAt = createdAt; record.batchNumber = batchNumber
        record.profile = profile
        if queued { record.state = .queued }
        initialRecord = record
        try Wire.encoder().encode(record).write(to: directory.appendingPathComponent("manifest.json"), options: .atomic)
    }
    public func snapshot() -> ScanRecord { record }
    public func markPreparing() throws {
        record.state = .preparing; record.startedAt = Date()
        try save()
    }
    public func markProcessing() throws {
        record.state = .processing
        try save()
    }
    public func markScanning(settings: [String: JSONValue]) throws {
        record.state = .scanning; record.appliedSettings = settings
        try save()
    }
    public func preserve(_ url: URL, sequence: Int) throws {
        try validateStorage()
        guard url.isFileURL, url.deletingLastPathComponent().standardizedFileURL == incomingDirectory,
              url.resolvingSymlinksInPath().path == url.standardizedFileURL.path else {
            throw ScannerError("storage", "Driver returned an unexpected output path; file left untouched: \(url.path)")
        }
        guard sequence > 0, !record.images.contains(where: { $0.sequence == sequence }) else {
            throw ScannerError("storage", "Duplicate or invalid image sequence; original left untouched.")
        }
        let attrs = try FileManager.default.attributesOfItem(atPath: url.path)
        guard attrs[.type] as? FileAttributeType == .typeRegular,
              let size = attrs[.size] as? NSNumber, size.intValue > 0, size.intValue <= 512 * 1024 * 1024 else {
            throw ScannerError("storage", "Driver output is empty, non-regular or exceeds 512 MiB; retained in incoming.")
        }
        let bytes = try Data(contentsOf: url, options: .mappedIfSafe)
        guard let source = CGImageSourceCreateWithData(bytes as CFData, nil),
              let uti = CGImageSourceGetType(source) as String?,
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let width = properties[kCGImagePropertyPixelWidth] as? Int,
              let height = properties[kCGImagePropertyPixelHeight] as? Int, width > 0, height > 0 else {
            throw ScannerError("image", "Driver output is not a readable image; retained in incoming.")
        }
        let expectedUTI = record.request.format == .tiff ? "public.tiff" : "public.png"
        guard uti == expectedUTI else { throw ScannerError("image", "Driver returned \(uti), expected \(expectedUTI); retained in incoming.") }
        let digest = SHA256.hash(data: bytes).map { String(format: "%02x", $0) }.joined()
        let relative = BatchNaming.originalName(sequence: sequence, format: record.request.format)
        let destination = directory.appendingPathComponent(relative)
        // copyItem refuses an existing destination. Keep incoming too, including on any error.
        try FileManager.default.copyItem(at: url, to: destination)
        let copied = try Data(contentsOf: destination, options: .mappedIfSafe)
        guard SHA256.hash(data: copied) == SHA256.hash(data: bytes) else {
            throw ScannerError("storage", "Original copy checksum mismatch; both files retained.")
        }
        record.images.append(OriginalImage(sequence: sequence, relativePath: relative, bytes: bytes.count,
            sha256: digest, width: width, height: height, imageCount: CGImageSourceGetCount(source), uniformType: uti))
        record.images.sort { $0.sequence < $1.sequence }
        try save()
    }
    public func finish(errors: [ScannerError], cancelled: Bool, sessionClosed: Bool) throws {
        record.errors += errors; record.sessionClosed = sessionClosed
        if record.images.isEmpty && record.errors.isEmpty && !cancelled {
            record.errors.append(ScannerError("noImages", "Scanner completed without delivering images."))
        }
        if record.request.duplex && record.images.count % 2 != 0 {
            record.warnings.append("Odd duplex image count; do not pair automatically.")
        }
        record.state = !record.errors.isEmpty ? .failed : (cancelled ? .cancelled : .completed)
        record.completedAt = Date()
        try save()
    }
    public func recordPersistenceFailure(_ error: Error) {
        record.errors.append(ScannerError("storage", error: error)); record.state = .failed
        record.completedAt = Date()
    }
    private func save() throws {
        try validateStorage()
        try Wire.encoder().encode(record).write(to: directory.appendingPathComponent("manifest.json"), options: .atomic)
        try persist(record)
    }
}
