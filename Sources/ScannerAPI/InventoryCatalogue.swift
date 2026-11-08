import Foundation

public struct InventoryCatalogueSource: Codable, Hashable, Sendable {
    public var provider: String
    public var setID: String
    public var language: CardLanguage
    public init(provider: String, setID: String, language: CardLanguage) {
        self.provider = provider; self.setID = setID; self.language = language
    }
}
public struct InventoryCatalogueCard: Codable, Sendable {
    public var id: String
    public var name: String
    public var number: String
    public var language: String
    public var thumbnailKey: String?
    public init(id: String, name: String, number: String, language: String, thumbnailKey: String? = nil) {
        self.id = id; self.name = name; self.number = number; self.language = language; self.thumbnailKey = thumbnailKey
    }
}
public struct InventoryMissingCards: Codable, Sendable {
    public var schemaVersion = 1
    public var cards: [InventoryCatalogueCard]
    public var catalogueCount: Int?
    public var note: String?
    public init(cards: [InventoryCatalogueCard] = [], catalogueCount: Int? = nil, note: String? = nil) {
        self.cards = cards; self.catalogueCount = catalogueCount; self.note = note
    }
}
