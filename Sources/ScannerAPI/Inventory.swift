import Foundation

public struct InventoryCopy: Codable, Sendable {
    public var batchID: UUID
    public var batchNumber: Int?
    public var cardNumber: Int
    public init(batchID: UUID, batchNumber: Int?, cardNumber: Int) {
        self.batchID = batchID; self.batchNumber = batchNumber; self.cardNumber = cardNumber
    }
}
public struct InventoryPreview: Codable, Sendable {
    public var batchID: UUID
    public var image: OriginalImage
    public var editID: UUID?
    public init(batchID: UUID, image: OriginalImage, editID: UUID?) {
        self.batchID = batchID; self.image = image; self.editID = editID
    }
}
public struct InventorySet: Codable, Sendable {
    public var id: String
    public var name: String
    public var game: CardGame
    public var quantity: Int
    public var entries: Int
    public var previews: [InventoryPreview]
    public init(id: String, name: String, game: CardGame, quantity: Int, entries: Int, previews: [InventoryPreview]) {
        self.id = id; self.name = name; self.game = game; self.quantity = quantity; self.entries = entries; self.previews = previews
    }
}
public struct InventoryEntry: Codable, Sendable {
    public var id: String
    public var game: CardGame
    public var language: String
    public var name: String
    public var setName: String?
    public var number: String?
    public var status: String
    public var quantity: Int
    public var confirmedQuantity: Int
    public var preview: InventoryPreview?
    public var setKey: String
    public var copies: [InventoryCopy]
    public init(id: String, game: CardGame, language: String, name: String, setName: String?, number: String?, status: String, setKey: String, copy: InventoryCopy, confirmed: Bool) {
        self.id = id; self.game = game; self.language = language; self.name = name
        self.setName = setName; self.number = number; self.status = status
        self.setKey = setKey
        self.quantity = 1; self.confirmedQuantity = confirmed ? 1 : 0; self.copies = [copy]
    }
}
public struct InventoryPage: Codable, Sendable {
    public var schemaVersion = 1
    public var sets: [InventorySet] = []
    public var entries: [InventoryEntry]
    public var totalCopies: Int
    public var catalogueCopies: Int
    public var awaitingCatalogueCopies: Int
    public var unresolvedCopies: Int
    public var excludedCodeCards: Int
    public var matchingEntries: Int
    public var nextOffset: Int?
    public init(entries: [InventoryEntry], totalCopies: Int, catalogueCopies: Int, awaitingCatalogueCopies: Int, unresolvedCopies: Int, excludedCodeCards: Int, matchingEntries: Int, nextOffset: Int?) {
        self.entries = entries; self.totalCopies = totalCopies; self.catalogueCopies = catalogueCopies
        self.awaitingCatalogueCopies = awaitingCatalogueCopies; self.unresolvedCopies = unresolvedCopies
        self.excludedCodeCards = excludedCodeCards; self.matchingEntries = matchingEntries; self.nextOffset = nextOffset
    }
}
