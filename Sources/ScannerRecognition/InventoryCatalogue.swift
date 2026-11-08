import Foundation
import ScannerAPI

public protocol InventoryCatalogueProviding: Sendable {
    func cards(in source: InventoryCatalogueSource) async throws -> [InventoryCatalogueCard]
    func thumbnail(key: String) async throws -> ImageThumbnail
}

/// Catalogue lists are separate from scan inventory: no synthetic stock or scans.
public actor InventoryCatalogue: InventoryCatalogueProviding {
    private let pokemon: [CardLanguage: TCGdexClient]
    private let yugioh: YugiohCatalogue
    private let starwars: StarWarsCatalogue
    private let thumbnails: CatalogueThumbnails
    private var cache: [InventoryCatalogueSource: (Date, [InventoryCatalogueCard])] = [:]
    private var pending: [InventoryCatalogueSource: Task<[InventoryCatalogueCard], Error>] = [:]
    public init() {
        pokemon = [.english: TCGdexClient(), .korean: TCGdexClient(language: .korean)]
        yugioh = YugiohCatalogue(); starwars = StarWarsCatalogue(); thumbnails = CatalogueThumbnails()
    }
    init(pokemon: [CardLanguage: TCGdexClient], yugioh: YugiohCatalogue, starwars: StarWarsCatalogue, thumbnails: CatalogueThumbnails) {
        self.pokemon = pokemon; self.yugioh = yugioh; self.starwars = starwars; self.thumbnails = thumbnails
    }
    public func cards(in source: InventoryCatalogueSource) async throws -> [InventoryCatalogueCard] {
        if let hit = cache[source], Date().timeIntervalSince(hit.0) < 86400 { return hit.1 }
        if let task = pending[source] { return try await task.value }
        guard pending.count < 4 else { throw ScannerError("resourceLimit", "Catalogue is busy; retry shortly.") }
        let task = Task { try await self.load(source) }
        pending[source] = task
        defer { pending[source] = nil }
        let cards = try await task.value
        if cache.count >= 64 { cache.removeAll() }
        cache[source] = (Date(), cards)
        return cards
    }
    private func load(_ source: InventoryCatalogueSource) async throws -> [InventoryCatalogueCard] {
        var result: [InventoryCatalogueCard] = []
        switch source.provider {
        case "tcgdex":
            guard let client = pokemon[source.language] else { throw ScannerError("unsupported", "No catalogue for this language.") }
            for card in try await client.setCards(source.setID) {
                let url = card.image.flatMap { URL(string: $0 + "/low.png") }
                result.append(InventoryCatalogueCard(id: card.id, name: card.name, number: card.localId, language: source.language.rawValue,
                    thumbnailKey: await thumbnails.register(url)))
            }
        case "ygoprodeck" where source.language == .english:
            let cards = try await yugioh.catalogue().cards
            for card in cards {
                let printings = Set((card.card_sets ?? []).map(\.set_code).filter { $0.split(separator: "-").first.map(String.init) == source.setID && $0.range(of: #"-(EN|AE)?[0-9]+$"#, options: .regularExpression) != nil })
                for code in printings.sorted() {
                    let url = card.card_images?.first.flatMap { URL(string: "https://images.ygoprodeck.com/images/cards/\($0.id).jpg") }
                    result.append(InventoryCatalogueCard(id: "ygoprodeck:\(card.id):\(code)", name: card.name, number: code, language: "en",
                        thumbnailKey: await thumbnails.register(url)))
                }
            }
        case "swu-db" where source.language == .english:
            for card in try await starwars.cards(set: source.setID) {
                result.append(InventoryCatalogueCard(id: "swu-db:" + card.Set + "-" + card.Number,
                    name: card.Name + (card.Subtitle.map { " — " + $0 } ?? ""), number: card.Number, language: "en",
                    thumbnailKey: await thumbnails.register(card.FrontArt.flatMap(URL.init(string:)))))
            }
        default: throw ScannerError("unsupported", "A set checklist is not available for this catalogue and language.")
        }
        guard !result.isEmpty, result.count <= 5000, Set(result.map(\.id)).count == result.count else {
            throw ScannerError("catalogueUnavailable", "The catalogue does not have a usable card list for this set yet.")
        }
        return result.sorted { $0.number.localizedStandardCompare($1.number) == .orderedAscending }
    }
    public func thumbnail(key: String) async throws -> ImageThumbnail { try await thumbnails.thumbnail(key: key) }
}
