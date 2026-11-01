import Foundation
import GRDB
import ScannerAPI

extension ScannerDatabase {
    public func cachedPrices(_ cardID: String) throws -> CardMarketPrices? {
        try priceCache("card-variants-v2:" + cardID, as: CardMarketPrices.self)
    }
    public func cachedSterlingRate() throws -> EuroSterlingRate? {
        try priceCache("eur-gbp", as: EuroSterlingRate.self)
    }
    public func cachePrices(_ value: CardMarketPrices) throws {
        try savePriceCache("card-variants-v2:" + value.cardID, date: value.fetchedAt, value: value)
    }
    public func cacheSterlingRate(_ value: EuroSterlingRate) throws {
        try savePriceCache("eur-gbp", date: value.fetchedAt, value: value)
    }
    private func priceCache<T: Decodable>(_ key: String, as: T.Type) throws -> T? {
        try queue.read { db in
            try Data.fetchOne(db, sql: "SELECT record FROM marketPriceCache WHERE key = ?", arguments: [key])
                .map { try Wire.decoder().decode(T.self, from: $0) }
        }
    }
    private func savePriceCache<T: Encodable>(_ key: String, date: Date, value: T) throws {
        try queue.write { db in
            try db.execute(sql: "INSERT OR REPLACE INTO marketPriceCache VALUES (?, ?, ?)", arguments: [key, date.timeIntervalSince1970, try Wire.encoder().encode(value)])
            try db.execute(sql: "DELETE FROM marketPriceCache WHERE key IN (SELECT key FROM marketPriceCache ORDER BY fetched DESC LIMIT -1 OFFSET 5000)")
        }
    }
}
