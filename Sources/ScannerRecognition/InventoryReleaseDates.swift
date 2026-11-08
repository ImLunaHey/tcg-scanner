import Foundation
import ScannerAPI

public struct InventoryReleaseDateSnapshot: Sendable {
    public var dates: [InventoryCatalogueSource: String]
    public var pending: Bool
    public init(dates: [InventoryCatalogueSource: String] = [:], pending: Bool = false) { self.dates = dates; self.pending = pending }
}

/// Inventory reads never wait for remote metadata. Three background GETs at most;
/// successful/unknown dates are cached for a day, failures retry after five minutes.
actor InventoryReleaseDates {
    private struct Entry: Codable {
        var source: InventoryCatalogueSource
        var date: String?
        var expires: Date
    }
    private let file: URL
    private let fetch: @Sendable (InventoryCatalogueSource) async throws -> String?
    private var entries: [InventoryCatalogueSource: Entry] = [:]
    private var loaded = false
    private var queued: [InventoryCatalogueSource] = []
    private var active = Set<InventoryCatalogueSource>()
    init(file: URL = FileManager.default.homeDirectoryForCurrentUser.appendingPathComponent("Library/Caches/Scanner/set-release-dates.json"),
         fetch: (@Sendable (InventoryCatalogueSource) async throws -> String?)? = nil) {
        self.file = file
        let pokemon: [CardLanguage: TCGdexClient] = [.english: TCGdexClient(), .korean: TCGdexClient(language: .korean)]
        let starwars = StarWarsCatalogue(), yugioh = YugiohCatalogue()
        self.fetch = fetch ?? { source in
            switch source.provider {
            case "tcgdex": return try await pokemon[source.language]?.releaseDate(source.setID)
            case "swu-db": return try await starwars.sets().first { $0.setId == source.setID }?.releaseDate
            case "ygoprodeck": return try await yugioh.setReleaseDate(source.setID)
            default: return nil
            }
        }
    }
    func snapshot(_ sources: [InventoryCatalogueSource]) -> InventoryReleaseDateSnapshot {
        guard !sources.isEmpty else { return InventoryReleaseDateSnapshot() }
        if !loaded {
            loaded = true
            if let info = try? file.resourceValues(forKeys: [.isRegularFileKey, .isSymbolicLinkKey, .fileSizeKey]),
               info.isRegularFile == true, info.isSymbolicLink != true, (info.fileSize ?? Int.max) <= 512 * 1024,
               let data = try? Data(contentsOf: file), let records = try? JSONDecoder().decode([Entry].self, from: data), records.count <= 2000 {
                for record in records { entries[record.source] = record }
            }
        }
        for source in Set(sources) where entries[source]?.expires ?? .distantPast < Date() {
            if !active.contains(source), !queued.contains(source), queued.count < 2000 { queued.append(source) }
        }
        pump()
        let dates = Dictionary(uniqueKeysWithValues: Set(sources).compactMap { source in entries[source]?.date.flatMap(Self.normalized).map { (source, $0) } })
        return InventoryReleaseDateSnapshot(dates: dates, pending: sources.contains { active.contains($0) || queued.contains($0) })
    }
    private func pump() {
        while active.count < 3, !queued.isEmpty {
            let source = queued.removeFirst()
            active.insert(source)
            Task(priority: .utility) {
                let entry: Entry
                do { entry = Entry(source: source, date: try await fetch(source).flatMap(Self.normalized), expires: Date().addingTimeInterval(86400)) }
                catch { entry = Entry(source: source, date: entries[source]?.date, expires: Date().addingTimeInterval(300)) }
                entries[source] = entry
                if entries.count > 2000 { entries = entries.filter { $0.value.expires > Date() } }
                persist()
                active.remove(source)
                pump()
            }
        }
    }
    private func persist() {
        let root = file.deletingLastPathComponent()
        guard entries.count <= 2000, let data = try? JSONEncoder().encode(Array(entries.values)), data.count <= 512 * 1024 else { return }
        do {
            try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true, attributes: [.posixPermissions: 0o700])
            guard root.standardizedFileURL.resolvingSymlinksInPath() == root.standardizedFileURL,
                  (try? file.resourceValues(forKeys: [.isSymbolicLinkKey]).isSymbolicLink) != true else { return }
            try data.write(to: file, options: .atomic)
        } catch { /* An unwritable optional cache must not affect inventory or scans. */ }
    }
    static func normalized(_ raw: String) -> String? {
        let fields: [Int]
        if raw.range(of: #"^\d{4}-\d{2}-\d{2}$"#, options: .regularExpression) != nil { fields = raw.split(separator: "-").compactMap { Int($0) } }
        else if raw.range(of: #"^\d{1,2}/\d{1,2}/\d{2,4}$"#, options: .regularExpression) != nil {
            let parts = raw.split(separator: "/").compactMap { Int($0) }
            fields = [parts[2] < 100 ? 2000 + parts[2] : parts[2], parts[0], parts[1]]
        } else { return nil }
        guard fields.count == 3, (1990...2100).contains(fields[0]), (1...12).contains(fields[1]), (1...31).contains(fields[2]) else { return nil }
        let calendar = Calendar(identifier: .gregorian)
        let components = DateComponents(year: fields[0], month: fields[1], day: fields[2])
        guard let date = calendar.date(from: components), calendar.dateComponents([.year, .month, .day], from: date) == components else { return nil }
        return String(format: "%04d-%02d-%02d", fields[0], fields[1], fields[2])
    }
}
