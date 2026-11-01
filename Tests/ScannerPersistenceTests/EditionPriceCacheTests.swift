import Foundation
import Testing
import ScannerAPI
@testable import ScannerPersistence

@Test func editionAwarePriceCacheNeverReadsPreEditionTransformedQuotes() throws {
    let db = try ScannerDatabase()
    // Deliberately invalid old-schema bytes also prove old keys are not decoded.
    try db.queue.write { database in
        try database.execute(sql: "INSERT INTO marketPriceCache VALUES (?, ?, ?)",
            arguments: ["card:sv02-097", Date().timeIntervalSince1970, Data("old quote format".utf8)])
    }
    #expect(try db.cachedPrices("sv02-097") == nil)
    let quotes = CardMarketPrices(cardID: "sv02-097", variants: [
        MarketVariantPrice(finish: .holo, euros: 1, variantID: "pumpkin", stamps: ["trick-or-trade"])])
    try db.cachePrices(quotes)
    let cached = try #require(try db.cachedPrices("sv02-097"))
    #expect(cached.variants.first?.variantID == "pumpkin")
    #expect(cached.variants.first?.stamps == ["trick-or-trade"])
}
