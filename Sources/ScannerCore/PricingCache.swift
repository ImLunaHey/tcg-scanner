import Foundation
import ScannerAPI
import ScannerPersistence
import ScannerRecognition

/// Bounded background fetching. API reads never wait for a public network request.
actor PricingCache {
    struct Snapshot: Sendable {
        var prices: [String: CardMarketPrices]
        var rate: EuroSterlingRate?
        var pending: Set<String>
        var ratePending: Bool
    }
    private let database: ScannerDatabase
    private let provider: any CardPricingProviding
    private var queued: [String] = []
    private var active: Set<String> = []
    private var retries: [String: Date] = [:]
    static let ttl: TimeInterval = 6 * 3600
    init(database: ScannerDatabase, provider: any CardPricingProviding) {
        self.database = database; self.provider = provider
    }
    func snapshot(_ ids: Set<String>) throws -> Snapshot {
        let now = Date()
        retries = retries.filter { $0.value > now }
        var prices: [String: CardMarketPrices] = [:]
        let rate = try database.cachedSterlingRate()
        if !ids.isEmpty, rate == nil || now.timeIntervalSince(rate!.fetchedAt) > Self.ttl { enqueue("eur-gbp") }
        for id in ids.sorted() {
            let value = try database.cachedPrices(id)
            if let value { prices[id] = value }
            if value == nil || now.timeIntervalSince(value!.fetchedAt) > Self.ttl { enqueue("card:" + id) }
        }
        startWorkers()
        let pending = active.union(queued)
        return Snapshot(prices: prices, rate: rate, pending: Set(ids.filter { pending.contains("card:" + $0) }), ratePending: pending.contains("eur-gbp"))
    }
    private func enqueue(_ key: String) {
        guard !active.contains(key), !queued.contains(key), retries[key] == nil, queued.count < 200,
              retries.count < 1000 else { return }
        queued.append(key)
    }
    private func startWorkers() {
        while active.count < 2, !queued.isEmpty {
            let key = queued.removeFirst()
            active.insert(key)
            Task { await fetch(key) }
        }
    }
    private func fetch(_ key: String) async {
        do {
            if key == "eur-gbp" { try database.cacheSterlingRate(await provider.sterlingRate()) }
            else {
                let id = String(key.dropFirst(5))
                let value = try await provider.prices(cardID: id)
                guard value.cardID == id else { throw ScannerError("pricing", "Mismatched catalogue ID.") }
                try database.cachePrices(value)
            }
        } catch {
            // Cached quotes remain available, explicitly marked stale. Missing quotes
            // are unavailable, never zero. Cooldown prevents an offline retry storm.
            retries[key] = Date().addingTimeInterval(60)
        }
        active.remove(key)
        startWorkers()
    }
}
