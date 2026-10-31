import ScannerAPI

/// Disposable derived previews, bounded to 32 MiB. Original reads never use this cache.
struct ThumbnailCache {
    private struct Entry {
        let fingerprint: String
        let thumbnail: ImageThumbnail
    }
    private var entries: [String: Entry] = [:]
    private var order: [String] = []
    private var bytes = 0
    let limit: Int
    init(limit: Int = 32 * 1024 * 1024) { self.limit = limit }

    mutating func value(key: String, fingerprint: String) -> ImageThumbnail? {
        guard let entry = entries[key] else { return nil }
        guard entry.fingerprint == fingerprint else { remove(key); return nil }
        order.removeAll { $0 == key }; order.append(key)
        return entry.thumbnail
    }
    mutating func insert(_ thumbnail: ImageThumbnail, key: String, fingerprint: String) {
        remove(key)
        let count = thumbnail.base64.utf8.count
        guard count <= limit else { return }
        while bytes + count > limit, let oldest = order.first { remove(oldest) }
        entries[key] = Entry(fingerprint: fingerprint, thumbnail: thumbnail)
        order.append(key); bytes += count
    }
    private mutating func remove(_ key: String) {
        if let old = entries.removeValue(forKey: key) { bytes -= old.thumbnail.base64.utf8.count }
        order.removeAll { $0 == key }
    }
}
