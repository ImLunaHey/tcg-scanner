import Foundation

/// Public catalogue quotes, separate from immutable capture/recognition metadata.
public struct MarketVariantPrice: Codable, Sendable {
    /// Scoped to CardMarketPrices.cardID; a variant ID is not globally unique.
    public var variantID: String?
    public var stamps: [String]
    public var finish: CardFinish
    public var pattern: String?
    public var productID: Int?
    public var euros: Decimal?
    public var updated: String?
    public init(finish: CardFinish, pattern: String? = nil, productID: Int? = nil, euros: Decimal?, updated: String? = nil,
                variantID: String? = nil, stamps: [String] = []) {
        self.finish = finish; self.pattern = pattern; self.productID = productID; self.euros = euros; self.updated = updated
        self.variantID = variantID; self.stamps = stamps
    }
}

public struct CardMarketPrices: Codable, Sendable {
    public var cardID: String
    public var fetchedAt: Date
    public var variants: [MarketVariantPrice]
    public var requiresPrintingReview: Bool
    public init(cardID: String, fetchedAt: Date = Date(), variants: [MarketVariantPrice], requiresPrintingReview: Bool = false) {
        self.cardID = cardID; self.fetchedAt = fetchedAt; self.variants = variants; self.requiresPrintingReview = requiresPrintingReview
    }
}

public struct EuroSterlingRate: Codable, Sendable {
    public var poundsPerEuro: Decimal
    public var date: String
    public var fetchedAt: Date
    public init(poundsPerEuro: Decimal, date: String, fetchedAt: Date = Date()) {
        self.poundsPerEuro = poundsPerEuro; self.date = date; self.fetchedAt = fetchedAt
    }
}

public struct CardValuation: Codable, Sendable {
    public enum State: String, Codable, Sendable { case priced, range, pending, unavailable }
    public var number: Int
    public var cardID: String?
    public var editionKey: String?
    public var state: State = .unavailable
    /// Integer GBP minor units. Rounded per physical card before summing the batch.
    public var lowPence: Int?
    public var highPence: Int?
    public var stale = false
    public var reason: String
    public var quotes: [MarketVariantPrice] = []
    public init(number: Int, reason: String) { self.number = number; self.reason = reason }
}

public struct BatchValuation: Codable, Sendable {
    public var schemaVersion = 1
    public var batchID: UUID
    public var currency = "GBP"
    public var source = "Cardmarket via TCGdex"
    public var metric = "trend"
    public var exchangeSource = "European Central Bank"
    public var exchangeRate: EuroSterlingRate?
    public var cards: [CardValuation]
    public var pricedCount: Int
    public var totalCount: Int
    public var lowPence: Int?
    public var highPence: Int?
    public var refreshing: Bool
    public var stale: Bool
    public var warning = "Market guide for the matched catalogue printing; not condition-adjusted. Excludes fees and postage."
    public init(batchID: UUID, cards: [CardValuation], exchangeRate: EuroSterlingRate?, refreshing: Bool) {
        self.batchID = batchID; self.cards = cards; self.exchangeRate = exchangeRate; self.refreshing = refreshing
        totalCount = cards.count
        let priced = cards.filter { $0.lowPence != nil && $0.highPence != nil }
        pricedCount = priced.count
        lowPence = priced.isEmpty ? nil : priced.reduce(0) { $0 + ($1.lowPence ?? 0) }
        highPence = priced.isEmpty ? nil : priced.reduce(0) { $0 + ($1.highPence ?? 0) }
        stale = cards.contains { $0.stale }
    }
}
