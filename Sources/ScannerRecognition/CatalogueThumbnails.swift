import CryptoKit
import Foundation
import ImageIO
import ScannerAPI

/// Small, locally hosted catalogue previews. Only URLs from validated catalogue
/// responses can be registered; clients can request their opaque keys, not URLs.
actor CatalogueThumbnails {
    static let diskLimit = 256 * 1024 * 1024
    let root: URL
    private var urls: [String: URL] = [:]
    private var pending: [String: Task<ImageThumbnail, Error>] = [:]
    private let fetch: @Sendable (URL) async throws -> Data
    init(root: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Caches/Scanner/inventory-thumbnails"),
         fetch: (@Sendable (URL) async throws -> Data)? = nil) {
        self.root = root
        let network = CatalogueImageNetwork()
        self.fetch = fetch ?? { try await network.get($0) }
    }
    static func allowed(_ url: URL) -> Bool {
        guard let c = URLComponents(url: url, resolvingAgainstBaseURL: false), c.scheme == "https",
              c.port == nil, c.user == nil, c.password == nil, c.query == nil, c.fragment == nil,
              !c.percentEncodedPath.contains("%"), url.absoluteString.utf8.count <= 512 else { return false }
        let pattern: String
        switch c.host {
        case "assets.tcgdex.net": pattern = #"^/(en|ko)/[A-Za-z0-9._+-]+/[A-Za-z0-9._+-]+/[A-Za-z0-9._+-]+/low\.png$"#
        case "images.ygoprodeck.com": pattern = #"^/images/cards/[0-9]{1,10}\.jpg$"#
        case "cdn.swu-db.com": pattern = #"^/images/cards/[A-Z0-9]{3,8}/[A-Za-z0-9_-]+\.png$"#
        default: return false
        }
        return !c.path.split(separator: "/").contains(where: { $0 == "." || $0 == ".." })
            && c.path.range(of: pattern, options: .regularExpression) != nil
    }
    func register(_ url: URL?) -> String? {
        guard let url, Self.allowed(url) else { return nil }
        let key = Self.digest(Data(url.absoluteString.utf8))
        guard urls.count < 100_000 || urls[key] != nil else { return nil }
        urls[key] = url
        return key
    }
    func thumbnail(key: String) async throws -> ImageThumbnail {
        guard key.range(of: #"^[a-f0-9]{64}$"#, options: .regularExpression) != nil else { throw ScannerError("invalidParams", "Invalid catalogue thumbnail key.") }
        if let data = try cached(key), let image = try? Self.encode(data, alreadySmall: true) { return image }
        guard let url = urls[key] else { throw ScannerError("imageNotFound", "Reload the set checklist to request this catalogue image.") }
        if let task = pending[key] { return try await task.value }
        guard pending.count < 2 else { throw ScannerError("resourceLimit", "Catalogue previews are busy; retry shortly.") }
        let task = Task {
            let image = try Self.encode(await fetch(url))
            try save(image, key: key)
            return image
        }
        pending[key] = task
        defer { pending[key] = nil }
        return try await task.value
    }
    private func directory() throws {
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        guard root.standardizedFileURL.resolvingSymlinksInPath() == root.standardizedFileURL else { throw ScannerError("accessDenied", "Invalid catalogue cache directory.") }
    }
    private func cached(_ key: String) throws -> Data? {
        try directory()
        let file = root.appendingPathComponent(key + ".jpg")
        guard let info = try? file.resourceValues(forKeys: [.isSymbolicLinkKey, .isRegularFileKey, .fileSizeKey, .contentModificationDateKey]),
              info.isSymbolicLink != true, info.isRegularFile == true, let size = info.fileSize, size <= 512 * 1024,
              let date = info.contentModificationDate, Date().timeIntervalSince(date) < 7 * 86400 else { return nil }
        return try Data(contentsOf: file)
    }
    private func save(_ image: ImageThumbnail, key: String) throws {
        try directory()
        guard let data = Data(base64Encoded: image.base64) else { return }
        let files = try FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: [.fileSizeKey, .contentModificationDateKey, .isSymbolicLinkKey])
        var entries: [(URL, Int, Date)] = []
        for file in files where file.pathExtension == "jpg" {
            let info = try file.resourceValues(forKeys: [.fileSizeKey, .contentModificationDateKey, .isSymbolicLinkKey])
            guard info.isSymbolicLink != true else { continue }
            entries.append((file, info.fileSize ?? 0, info.contentModificationDate ?? .distantPast))
        }
        var used = entries.reduce(0) { $0 + $1.1 }
        for entry in entries.sorted(by: { $0.2 < $1.2 }) where used + data.count > Self.diskLimit {
            try FileManager.default.removeItem(at: entry.0); used -= entry.1
        }
        try data.write(to: root.appendingPathComponent(key + ".jpg"), options: .atomic)
    }
    private static func digest(_ data: Data) -> String { SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined() }
    static func encode(_ data: Data, alreadySmall: Bool = false) throws -> ImageThumbnail {
        guard data.count <= 8 * 1024 * 1024,
              let source = CGImageSourceCreateWithData(data as CFData, [kCGImageSourceShouldCache: false] as CFDictionary),
              CGImageSourceGetCount(source) == 1,
              let info = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let width = info[kCGImagePropertyPixelWidth] as? Int, let height = info[kCGImagePropertyPixelHeight] as? Int,
              width > 0, height > 0, width <= 8000, height <= 8000, width * height <= 16_000_000,
              let image = CGImageSourceCreateThumbnailAtIndex(source, 0, [kCGImageSourceCreateThumbnailFromImageAlways: true,
                  kCGImageSourceCreateThumbnailWithTransform: true, kCGImageSourceThumbnailMaxPixelSize: 512] as CFDictionary) else {
            throw ScannerError("reference", "Invalid catalogue image.")
        }
        if alreadySmall && width <= 512 && height <= 512 {
            return ImageThumbnail(data: data, sha256: digest(data), width: width, height: height, mediaType: "image/jpeg")
        }
        let buffer = NSMutableData()
        guard let destination = CGImageDestinationCreateWithData(buffer, "public.jpeg" as CFString, 1, nil) else { throw ScannerError("reference", "Cannot encode catalogue image.") }
        CGImageDestinationAddImage(destination, image, [kCGImageDestinationLossyCompressionQuality: 0.85] as CFDictionary)
        guard CGImageDestinationFinalize(destination), buffer.length <= 512 * 1024 else { throw ScannerError("reference", "Catalogue thumbnail is too large.") }
        let bytes = buffer as Data
        return ImageThumbnail(data: bytes, sha256: digest(bytes), width: image.width, height: image.height, mediaType: "image/jpeg")
    }
}
private final class CatalogueImageNetwork: NSObject, URLSessionTaskDelegate, Sendable {
    func get(_ url: URL) async throws -> Data {
        guard CatalogueThumbnails.allowed(url) else { throw ScannerError("reference", "Unsupported catalogue image host.") }
        let config = URLSessionConfiguration.ephemeral
        config.timeoutIntervalForRequest = 12; config.timeoutIntervalForResource = 20
        config.httpCookieStorage = nil; config.urlCache = nil
        let session = URLSession(configuration: config, delegate: self, delegateQueue: nil)
        defer { session.invalidateAndCancel() }
        let (stream, response) = try await session.bytes(from: url)
        guard let http = response as? HTTPURLResponse, http.statusCode == 200, http.url == url,
              ["image/png", "image/jpeg"].contains(http.mimeType ?? ""), response.expectedContentLength <= 8 * 1024 * 1024 else {
            throw ScannerError("referenceUnavailable", "Catalogue preview unavailable.")
        }
        var data = Data()
        for try await byte in stream {
            guard data.count < 8 * 1024 * 1024 else { throw ScannerError("resourceLimit", "Catalogue image is too large.") }
            data.append(byte)
        }
        return data
    }
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest) async -> URLRequest? { nil }
}
