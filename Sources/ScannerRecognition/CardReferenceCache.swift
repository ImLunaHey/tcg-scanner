import CryptoKit
import Foundation
import ImageIO
import ScannerAPI

public struct CardReferenceImage: Sendable {
    public let data: Data
    public let url: URL
    public let sha256: String
    public let cacheHit: Bool
    public init(data: Data, url: URL, sha256: String, cacheHit: Bool) {
        self.data = data; self.url = url; self.sha256 = sha256; self.cacheHit = cacheHit
    }
}

public protocol CardReferenceProviding: Sendable {
    func reference(cardID: String) async throws -> CardReferenceImage?
}

/// Caches the single base-card image exposed by TCGdex, not a finish-labelled photograph.
/// Only public metadata and asset GETs leave the process; scans are never uploaded.
public actor TCGdexReferenceCache: CardReferenceProviding {
    static let maximumImageBytes = 8 * 1024 * 1024
    static let maximumDiskBytes = 256 * 1024 * 1024
    static let lifetime: TimeInterval = 7 * 86400

    struct Download: Sendable {
        var data: Data
        var responseURL: URL
        var statusCode: Int = 200
        var mimeType: String? = "image/png"
    }
    struct Receipt: Codable {
        var cardID: String
        var url: URL
        var sha256: String
        var bytes: Int
        var fetchedAt: Date
        var accessedAt: Date
    }

    private let root: URL
    private let metadata: @Sendable (String) async throws -> String?
    private let download: @Sendable (URL) async throws -> Download
    private let now: @Sendable () -> Date
    private let maximumBytes: Int
    private let ttl: TimeInterval
    private var pending: [String: Task<CardReferenceImage?, any Error>] = [:]
    private var missing: [String: Date] = [:]

    public init(root: URL = FileManager.default.homeDirectoryForCurrentUser
        .appendingPathComponent("Library/Caches/Scanner/card-references", isDirectory: true)) {
        let catalogue = TCGdexClient(), network = ReferenceNetwork()
        self.root = root; self.metadata = { try await catalogue.detail($0).image }
        self.download = { try await network.fetch($0) }; self.now = { Date() }
        self.maximumBytes = Self.maximumDiskBytes; self.ttl = Self.lifetime
    }

    /// Injection is internal to tests; callers cannot change the production hosts or bounds.
    init(root: URL, maximumBytes: Int = maximumDiskBytes, ttl: TimeInterval = lifetime,
         now: @escaping @Sendable () -> Date = { Date() },
         metadata: @escaping @Sendable (String) async throws -> String?,
         download: @escaping @Sendable (URL) async throws -> Download) {
        self.root = root; self.maximumBytes = max(0, min(maximumBytes, Self.maximumDiskBytes))
        self.ttl = ttl; self.now = now; self.metadata = metadata; self.download = download
    }

    public func reference(cardID: String) async throws -> CardReferenceImage? {
        try Task.checkCancellation()
        guard Self.validID(cardID) else { throw ScannerError("reference", "Invalid reference card identifier.") }
        if let existing = pending[cardID] {
            let value = try await existing.value
            try Task.checkCancellation()
            return value
        }
        let task = Task { try await self.load(cardID) }
        pending[cardID] = task
        defer { pending[cardID] = nil }
        // The bounded fetch belongs to the cache, not any individual waiter.
        // A cancelled client must not cancel another client's shared download.
        let value = try await task.value
        try Task.checkCancellation()
        return value
    }

    private func load(_ cardID: String) async throws -> CardReferenceImage? {
        try validateRoot()
        let clock = now()
        missing = missing.filter { $0.value > clock }
        if missing[cardID] != nil { return nil }
        let key = Self.digest(Data(cardID.utf8))
        if let hit = try read(key: key, cardID: cardID, clock: clock) { return hit }
        try Task.checkCancellation()
        guard let base = try await metadata(cardID) else {
            // An absent catalogue image is not evidence that a stamp or finish is absent.
            if missing.count < 1000 { missing[cardID] = clock.addingTimeInterval(86400) }
            return nil
        }
        let url = try Self.assetURL(base)
        let fetched = try await download(url)
        try Task.checkCancellation()
        guard fetched.responseURL == url else { throw ScannerError("reference", "Reference image redirect was rejected.") }
        if fetched.statusCode == 404 {
            if missing.count < 1000 { missing[cardID] = clock.addingTimeInterval(86400) }
            return nil
        }
        guard fetched.statusCode == 200, fetched.mimeType?.lowercased() == "image/png" else {
            throw ScannerError("referenceUnavailable", "The catalogue reference image is unavailable.")
        }
        try Self.validate(fetched.data)
        let hash = Self.digest(fetched.data)
        let receipt = Receipt(cardID: cardID, url: url, sha256: hash, bytes: fetched.data.count,
                              fetchedAt: now(), accessedAt: now())
        try write(fetched.data, receipt: receipt, key: key)
        return CardReferenceImage(data: fetched.data, url: url, sha256: hash, cacheHit: false)
    }

    /// Only the documented base URL plus /high.png is accepted. Never guess variant image paths.
    static func assetURL(_ base: String) throws -> URL {
        guard base.utf8.count <= 512, let components = URLComponents(string: base),
              components.scheme == "https", components.host == "assets.tcgdex.net",
              components.port == nil, components.user == nil, components.password == nil,
              components.query == nil, components.fragment == nil,
              !components.percentEncodedPath.contains("%") else {
            throw ScannerError("reference", "Invalid catalogue reference image URL.")
        }
        let pieces = components.path.split(separator: "/", omittingEmptySubsequences: false)
        guard pieces.count == 5, pieces[0].isEmpty, pieces[1] == "en",
              pieces.dropFirst(2).allSatisfy({ Self.validID(String($0)) }),
              let url = URL(string: base + "/high.png") else {
            throw ScannerError("reference", "Invalid catalogue reference image path.")
        }
        return url
    }

    private static func validID(_ value: String) -> Bool {
        !value.isEmpty && value != "." && value != ".." && value.utf8.count <= 100 &&
        value.utf8.allSatisfy { (48...57).contains($0) || (65...90).contains($0) || (97...122).contains($0) || $0 == 45 || $0 == 46 || $0 == 95 }
    }
    private static func digest(_ data: Data) -> String { SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined() }

    private static func validate(_ data: Data) throws {
        guard data.count <= maximumImageBytes, data.starts(with: [137, 80, 78, 71, 13, 10, 26, 10]),
              let source = CGImageSourceCreateWithData(data as CFData, [kCGImageSourceShouldCache: false] as CFDictionary),
              CGImageSourceGetCount(source) == 1,
              let props = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let width = props[kCGImagePropertyPixelWidth] as? Int,
              let height = props[kCGImagePropertyPixelHeight] as? Int,
              width > 0, height > 0, width <= 20_000, height <= 20_000, width * height <= 40_000_000,
              CGImageSourceCreateThumbnailAtIndex(source, 0, [
                kCGImageSourceCreateThumbnailFromImageAlways: true,
                kCGImageSourceCreateThumbnailWithTransform: true,
                kCGImageSourceThumbnailMaxPixelSize: 1024,
                kCGImageSourceShouldCacheImmediately: true
              ] as CFDictionary) != nil else {
            throw ScannerError("reference", "Reference must be one valid PNG up to 8 MiB and 40 megapixels.")
        }
    }

    private func file(_ key: String, _ suffix: String) -> URL { root.appendingPathComponent(key + suffix) }
    private func validateRoot() throws {
        if let values = try? root.resourceValues(forKeys: [.isSymbolicLinkKey, .isDirectoryKey]),
           values.isSymbolicLink == true || values.isDirectory == false {
            throw ScannerError("reference", "The reference cache root must be a directory, not a symbolic link.")
        }
    }
    private func regularSize(_ url: URL, limit: Int) -> Int? {
        guard let values = try? url.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey, .fileSizeKey]),
              values.isRegularFile == true, values.isSymbolicLink != true, let size = values.fileSize,
              size >= 0, size <= limit else { return nil }
        return size
    }
    private func read(key: String, cardID: String, clock: Date) throws -> CardReferenceImage? {
        let receiptURL = file(key, ".json"), imageURL = file(key, ".png")
        guard regularSize(receiptURL, limit: 4096) != nil, regularSize(imageURL, limit: Self.maximumImageBytes) != nil else { return nil }
        do {
            let receipt = try JSONDecoder().decode(Receipt.self, from: Data(contentsOf: receiptURL))
            guard receipt.cardID == cardID, receipt.bytes <= Self.maximumImageBytes,
                  clock.timeIntervalSince(receipt.fetchedAt) >= 0, clock.timeIntervalSince(receipt.fetchedAt) < ttl,
                  receipt.url.absoluteString.hasSuffix("/high.png"),
                  try Self.assetURL(String(receipt.url.absoluteString.dropLast("/high.png".count))) == receipt.url else { return nil }
            let data = try Data(contentsOf: imageURL)
            guard data.count == receipt.bytes, Self.digest(data) == receipt.sha256 else {
                remove(key); return nil
            }
            try Self.validate(data)
            try? FileManager.default.setAttributes([.modificationDate: clock], ofItemAtPath: imageURL.path)
            return CardReferenceImage(data: data, url: receipt.url, sha256: receipt.sha256, cacheHit: true)
        } catch is CancellationError { throw CancellationError() }
        catch { remove(key); return nil }
    }
    private func remove(_ key: String) {
        try? FileManager.default.removeItem(at: file(key, ".json"))
        try? FileManager.default.removeItem(at: file(key, ".png"))
    }
    private func write(_ data: Data, receipt: Receipt, key: String) throws {
        let encoded = try JSONEncoder().encode(receipt)
        let required = data.count + encoded.count
        guard required <= maximumBytes else { return }
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        // Never follow a swapped cache directory or cache-file symlink.
        try validateRoot()
        remove(key)
        let entries = try FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: [.isRegularFileKey, .isSymbolicLinkKey, .fileSizeKey])
        let keys = Set(entries.compactMap { entry -> String? in
            let stem = entry.deletingPathExtension().lastPathComponent
            guard ["png", "json"].contains(entry.pathExtension), stem.count == 64,
                  stem.utf8.allSatisfy({ (48...57).contains($0) || (97...102).contains($0) }) else { return nil }
            return stem
        })
        var cached: [(key: String, size: Int, accessed: Date)] = []
        for stem in keys {
            guard let receiptBytes = regularSize(file(stem, ".json"), limit: 4096),
                  let imageBytes = regularSize(file(stem, ".png"), limit: Self.maximumImageBytes),
                  let info = try? JSONDecoder().decode(Receipt.self, from: Data(contentsOf: file(stem, ".json"))) else {
                // Recover interrupted writes and corrupt cache entries without following links.
                remove(stem); continue
            }
            let accessed = (try? file(stem, ".png").resourceValues(forKeys: [.contentModificationDateKey]))?.contentModificationDate
            cached.append((stem, receiptBytes + imageBytes, accessed ?? info.accessedAt))
        }
        var used = cached.reduce(0) { $0 + $1.size }
        for entry in cached.sorted(by: { $0.accessed < $1.accessed }) where used + required > maximumBytes {
            remove(entry.key); used -= entry.size
        }
        try Task.checkCancellation()
        try data.write(to: file(key, ".png"), options: .atomic)
        do { try encoded.write(to: file(key, ".json"), options: .atomic) }
        catch { remove(key); throw error }
    }
}

private final class ReferenceNetwork: Sendable {
    private let session: URLSession
    init() {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = 12; configuration.timeoutIntervalForResource = 20
        configuration.httpCookieStorage = nil; configuration.urlCache = nil
        configuration.httpMaximumConnectionsPerHost = 2
        session = URLSession(configuration: configuration, delegate: ReferenceNoRedirects(), delegateQueue: nil)
    }
    deinit { session.invalidateAndCancel() }
    func fetch(_ url: URL) async throws -> TCGdexReferenceCache.Download {
        var request = URLRequest(url: url)
        request.setValue("image/png", forHTTPHeaderField: "Accept")
        let (stream, response) = try await session.bytes(for: request)
        guard let http = response as? HTTPURLResponse, let finalURL = http.url, finalURL == url,
              response.expectedContentLength <= TCGdexReferenceCache.maximumImageBytes else {
            throw ScannerError("referenceUnavailable", "Unsuccessful or oversized reference image response.")
        }
        guard http.statusCode == 200 else { return .init(data: Data(), responseURL: finalURL, statusCode: http.statusCode, mimeType: http.mimeType) }
        var bytes = Data()
        for try await byte in stream {
            guard bytes.count < TCGdexReferenceCache.maximumImageBytes else { throw ScannerError("resourceLimit", "Reference image exceeds 8 MiB.") }
            bytes.append(byte)
        }
        return .init(data: bytes, responseURL: finalURL, statusCode: http.statusCode, mimeType: http.mimeType)
    }
}

private final class ReferenceNoRedirects: NSObject, URLSessionTaskDelegate, Sendable {
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest) async -> URLRequest? { nil }
}
