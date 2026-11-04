import Foundation
import ImageIO
import ScannerAPI

struct YugiohCard: Decodable, Sendable {
    struct Printing: Decodable, Sendable { var set_name: String; var set_code: String; var set_rarity: String }
    struct Image: Decodable, Sendable { var id: Int }
    var id: Int
    var name: String
    var type: String
    var card_sets: [Printing]?
    var card_images: [Image]?
}

/// Fixed public GETs only. OCR text and scan pixels never leave the Mac.
actor YugiohCatalogue {
    struct Snapshot: Sendable { var cards: [YugiohCard]; var stale: Bool }
    struct Envelope: Decodable { var data: [YugiohCard] }
    static let endpoint = URL(string: "https://db.ygoprodeck.com/api/v7/cardinfo.php")!
    static let catalogueLimit = 64 * 1024 * 1024
    static let imageLimit = 8 * 1024 * 1024
    static let diskImageLimit = 256 * 1024 * 1024
    private let root: URL
    private let fetch: @Sendable (URL, Int) async throws -> Data
    private var memory: Snapshot?
    private var refreshAfter = Date.distantPast
    private var pending: Task<Snapshot, any Error>?
    private var lastImageFetch = ContinuousClock.now - .seconds(1)

    init(root: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Caches/Scanner/ygoprodeck"),
         fetch: (@Sendable (URL, Int) async throws -> Data)? = nil) {
        self.root = root
        let network = YugiohNetwork()
        self.fetch = fetch ?? { try await network.get($0, limit: $1) }
    }

    func catalogue() async throws -> Snapshot {
        try Task.checkCancellation()
        if let memory, refreshAfter > Date() { return memory }
        if let pending { return try await pending.value }
        let task = Task { try await self.load() }
        pending = task
        defer { pending = nil }
        let value = try await task.value
        try Task.checkCancellation()
        return value
    }

    private func load() async throws -> Snapshot {
        try directory()
        let file = root.appendingPathComponent("catalogue.json")
        let disk = try read(file, limit: Self.catalogueLimit)
        let old = disk.flatMap { try? Self.decode($0) }
        let date = try? file.resourceValues(forKeys: [.contentModificationDateKey]).contentModificationDate
        if let old, let date, Date().timeIntervalSince(date) < 2 * 86400 {
            let value = Snapshot(cards: old, stale: false)
            memory = value; refreshAfter = date.addingTimeInterval(2 * 86400); return value
        }
        do {
            let data = try await fetch(Self.endpoint, Self.catalogueLimit)
            let cards = try Self.decode(data)
            try data.write(to: file, options: .atomic)
            try FileManager.default.setAttributes([.posixPermissions: 0o600], ofItemAtPath: file.path)
            let value = Snapshot(cards: cards, stale: false)
            memory = value; refreshAfter = Date().addingTimeInterval(2 * 86400); return value
        } catch {
            if error is CancellationError { throw error }
            guard let old else { throw error }
            let value = Snapshot(cards: old, stale: true)
            memory = value; refreshAfter = Date().addingTimeInterval(300); return value
        }
    }

    static func decode(_ data: Data) throws -> [YugiohCard] {
        guard data.count <= catalogueLimit else { throw ScannerError("resourceLimit", "Yu-Gi-Oh! catalogue exceeds 64 MiB.") }
        let cards = try JSONDecoder().decode(Envelope.self, from: data).data
        guard !cards.isEmpty, cards.count <= 100_000,
              cards.allSatisfy({ card in
                  card.id > 0 && card.id <= 9_999_999_999 && !card.name.isEmpty && card.name.utf8.count <= 256 && card.type.utf8.count <= 128 &&
                  (card.card_sets?.count ?? 0) <= 1000 && (card.card_images?.count ?? 0) <= 100 &&
                  (card.card_sets ?? []).allSatisfy { $0.set_code.utf8.count <= 40 && $0.set_name.utf8.count <= 256 && $0.set_rarity.utf8.count <= 128 }
              }), Set(cards.map(\.id)).count == cards.count else {
            throw ScannerError("catalogue", "Invalid Yu-Gi-Oh! catalogue metadata.")
        }
        return cards
    }

    /// Cache one base artwork for future local comparisons. It does not establish
    /// a particular foil treatment or printing, and is not hotlinked by clients.
    func cacheReference(_ card: YugiohCard) async throws {
        guard let id = card.card_images?.first?.id else { return }
        guard id > 0, id <= 9_999_999_999 else { throw ScannerError("catalogue", "Invalid artwork identifier.") }
        try directory()
        let file = root.appendingPathComponent("reference-\(id).jpg")
        if let cached = try read(file, limit: Self.imageLimit), Self.validImage(cached) {
            try FileManager.default.setAttributes([.modificationDate: Date()], ofItemAtPath: file.path)
            return
        }
        let due = lastImageFetch + .milliseconds(150)
        if due > .now { try await Task.sleep(until: due, clock: .continuous) }
        lastImageFetch = .now
        let url = URL(string: "https://images.ygoprodeck.com/images/cards/\(id).jpg")!
        let data = try await fetch(url, Self.imageLimit)
        guard data.count <= Self.imageLimit, Self.validImage(data) else { throw ScannerError("reference", "Invalid or oversized Yu-Gi-Oh! reference image.") }
        let files = try FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: [.fileSizeKey, .contentModificationDateKey, .isSymbolicLinkKey])
            .filter { $0.lastPathComponent.hasPrefix("reference-") && $0.pathExtension == "jpg" }
        var entries: [(URL, Int, Date)] = []
        for entry in files {
            let values = try entry.resourceValues(forKeys: [.fileSizeKey, .contentModificationDateKey, .isSymbolicLinkKey])
            guard values.isSymbolicLink != true else { continue }
            entries.append((entry, values.fileSize ?? 0, values.contentModificationDate ?? .distantPast))
        }
        var used = entries.reduce(0) { $0 + $1.1 }
        for entry in entries.sorted(by: { $0.2 < $1.2 }) where used + data.count > Self.diskImageLimit {
            try FileManager.default.removeItem(at: entry.0); used -= entry.1
        }
        try Task.checkCancellation()
        try data.write(to: file, options: .atomic)
    }

    static func validImage(_ data: Data) -> Bool {
        guard let source = CGImageSourceCreateWithData(data as CFData, nil), CGImageSourceGetCount(source) == 1,
              let properties = CGImageSourceCopyPropertiesAtIndex(source, 0, nil) as? [CFString: Any],
              let width = properties[kCGImagePropertyPixelWidth] as? Int, let height = properties[kCGImagePropertyPixelHeight] as? Int else { return false }
        return width >= 100 && height >= 100 && width <= 8000 && height <= 8000 && width * height <= 16_000_000
    }
    private func directory() throws {
        try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
        guard try root.resourceValues(forKeys: [.isSymbolicLinkKey]).isSymbolicLink != true else { throw ScannerError("accessDenied", "Catalogue cache cannot be a symbolic link.") }
    }
    private func read(_ file: URL, limit: Int) throws -> Data? {
        guard FileManager.default.fileExists(atPath: file.path) else { return nil }
        let values = try file.resourceValues(forKeys: [.isSymbolicLinkKey, .isRegularFileKey, .fileSizeKey])
        guard values.isSymbolicLink != true, values.isRegularFile == true else { throw ScannerError("accessDenied", "Invalid catalogue cache file.") }
        guard let size = values.fileSize, size <= limit else { return nil }
        let data = try Data(contentsOf: file)
        return data.count <= limit ? data : nil
    }
}

private final class YugiohNetwork: Sendable {
    private let session: URLSession
    init() {
        let config = URLSessionConfiguration.ephemeral
        config.timeoutIntervalForRequest = 20; config.timeoutIntervalForResource = 60
        config.httpCookieStorage = nil; config.urlCache = nil
        session = URLSession(configuration: config, delegate: YugiohNoRedirects(), delegateQueue: nil)
    }
    deinit { session.invalidateAndCancel() }
    func get(_ url: URL, limit: Int) async throws -> Data {
        var request = URLRequest(url: url)
        request.setValue("Scanner/1 (local card catalogue)", forHTTPHeaderField: "User-Agent")
        let (stream, response) = try await session.bytes(for: request)
        guard let http = response as? HTTPURLResponse, http.statusCode == 200, http.url == url,
              response.expectedContentLength <= limit else { throw ScannerError("catalogueUnavailable", "Yu-Gi-Oh! catalogue request failed or exceeded its size limit.") }
        var bytes = Data()
        for try await byte in stream {
            guard bytes.count < limit else { throw ScannerError("resourceLimit", "Catalogue response exceeds its size limit.") }
            bytes.append(byte)
        }
        return bytes
    }
}

private final class YugiohNoRedirects: NSObject, URLSessionTaskDelegate, Sendable {
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse, newRequest request: URLRequest) async -> URLRequest? { nil }
}
