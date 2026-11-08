import Foundation
import ScannerAPI
import ScannerRecognition

/// Counts acquisition pairs, never image edits or OCR attempts. Rescans in a
/// different batch intentionally remain additional copies.
struct InventoryBuilder: Sendable {
    var catalogueSources: [String: Set<InventoryCatalogueSource>] = [:]
    var ownedCatalogueIDs: [InventoryCatalogueSource: Set<String>] = [:]
    var entries: [String: InventoryEntry] = [:]
    var excludedCodeCards = 0
    mutating func add(_ record: ScanRecord) {
        guard record.state.terminal,
              record.request.prepareCards == true || record.processedImages?.contains(where: { $0.cardPreparation != nil }) == true else { return }
        let groups = Dictionary(grouping: record.images, by: { record.request.duplex ? ($0.sequence + 1) / 2 : $0.sequence })
        let detections = CardGameDetection.resolve(record)
        for number in groups.keys.sorted() {
            let images = groups[number]!
            let detection = record.request.duplex ? detections.first { $0.number == number } : nil
            if detection?.contentKind == .pokemonCodeCard { excludedCodeCards += 1; continue }
            let game = detection?.game ?? .unknown
            let language = record.request.resolvedCardLanguage.rawValue
            let copy = InventoryCopy(batchID: record.id, batchNumber: record.batchNumber, cardNumber: number)
            let identities = (record.cardIdentifications ?? []).filter { identity in
                images.contains { $0.sequence == identity.request.sequence } &&
                identity.request.language == language &&
                (detection?.frontSequence == nil || detection?.frontSequence == identity.request.sequence)
            }
            let matched = identities.compactMap { identity -> (CardIdentification, CardCandidate)? in
                ScannerCoordinator.pricedCandidate(identity).map { (identity, $0) }
            }
            let waiting = identities.filter { $0.state == .awaitingCatalogue && $0.evidence?.uncataloguedCard != nil }
            let complete = !record.request.duplex || images.count == 2
            let base = [game.rawValue, language]
            var entry: InventoryEntry
            if complete, game != .unknown, matched.count == 1, let (identity, card) = matched.first {
                let id = key(base + ["catalogue", card.provider ?? "tcgdex", card.id])
                let source = InventoryCatalogueSource(provider: card.provider ?? "tcgdex", setID: catalogueSetID(card), language: record.request.resolvedCardLanguage)
                catalogueSources[catalogueSetKey(card, game: game), default: []].insert(source)
                ownedCatalogueIDs[source, default: []].insert(card.id)
                entry = InventoryEntry(id: id, game: game, language: language, name: card.name, setName: card.setName,
                    number: card.localID + (card.officialSetCount.map { "/\($0)" } ?? ""), status: "catalogue", setKey: catalogueSetKey(card, game: game), copy: copy, confirmed: identity.state == .confirmed)
            } else if complete, game != .unknown, matched.isEmpty, waiting.count == 1, let printed = waiting.first?.evidence?.uncataloguedCard {
                let id = key(base + ["awaitingCatalogue", printed.setCode, printed.collectorNumber, printed.name.lowercased()])
                entry = InventoryEntry(id: id, game: game, language: language, name: printed.name,
                    setName: printed.setName ?? printed.setCode, number: printed.collectorNumber, status: "awaitingCatalogue", setKey: game.rawValue + "-printed-" + printed.setCode, copy: copy, confirmed: false)
            } else {
                let id = key(["unresolved", record.id.uuidString, String(number)])
                entry = InventoryEntry(id: id, game: game, language: language,
                    name: complete ? "Needs identification" : "Incomplete card pair", setName: nil, number: nil,
                    status: "unresolved", setKey: game.rawValue + "-unidentified", copy: copy, confirmed: false)
            }
            let sequence = detection?.frontSequence ?? matched.first?.0.request.sequence
                ?? waiting.first?.request.sequence ?? images.map(\.sequence).min()
            if let original = images.first(where: { $0.sequence == sequence }) {
                let prepared = record.processedImages?.last {
                    $0.request.sequence == original.sequence && $0.cardPreparation != nil && $0.sourceSHA256 == original.sha256
                }
                entry.preview = InventoryPreview(batchID: record.id, image: prepared?.image ?? original, editID: prepared?.id)
            }
            if var existing = entries[entry.id] {
                existing.quantity += 1; existing.confirmedQuantity += entry.confirmedQuantity
                existing.copies.append(copy); entry = existing
            }
            entries[entry.id] = entry
        }
    }
    private func catalogueSetID(_ card: CardCandidate) -> String {
        // YGOPRODeck stores a full printing code (DUEA-EN001); the prefix
        // identifies the set shared by its other cards.
        return card.provider == "ygoprodeck"
            ? card.setID.split(separator: "-").first.map(String.init) ?? "unidentified"
            : card.setID
    }
    private func catalogueSetKey(_ card: CardCandidate, game: CardGame) -> String {
        game.rawValue + "-" + (card.provider ?? "tcgdex") + "-" + catalogueSetID(card)
    }
    private func setSlugs() -> [String: String] {
        let groups = Dictionary(grouping: entries.values, by: \.setKey)
        let names = groups.mapValues { values in
            slug(values.sorted { $0.id < $1.id }[0].setName ?? "Unidentified")
        }
        let counts = Dictionary(grouping: names.keys, by: { names[$0]! })
        var result: [String: String] = [:]
        var used = Set<String>()
        for key in groups.keys.sorted() {
            let name = names[key]!
            let game = groups[key]![0].game.rawValue
            let base = counts[name]!.count == 1 ? name : name + "-" + slug(game)
            var value = base
            var suffix = 2
            while used.contains(value) { value = base + "-\(suffix)"; suffix += 1 }
            used.insert(value)
            result[key] = value
        }
        return result
    }
    private func slug(_ name: String) -> String {
        let latin = (name.applyingTransform(.toLatin, reverse: false) ?? name)
            .folding(options: [.diacriticInsensitive, .caseInsensitive], locale: Locale(identifier: "en_US_POSIX"))
            .lowercased()
        let words = latin.components(separatedBy: CharacterSet(charactersIn: "abcdefghijklmnopqrstuvwxyz0123456789").inverted)
            .filter { !$0.isEmpty }
        let value = words.joined(separator: "-")
        return value.isEmpty ? "unidentified" : String(value.prefix(200))
    }
    func catalogueSource(set: String) throws -> InventoryCatalogueSource? {
        guard let key = setSlugs().first(where: { $0.value == set })?.key else { throw ScannerError("notFound", "Inventory set not found.") }
        return catalogueSources[key]?.sorted { $0.language.rawValue < $1.language.rawValue }.first
    }
    func missingCards(_ cards: [InventoryCatalogueCard], source: InventoryCatalogueSource) -> InventoryMissingCards {
        let owned = ownedCatalogueIDs[source] ?? []
        return InventoryMissingCards(cards: cards.filter { !owned.contains($0.id) }, catalogueCount: cards.count)
    }
    private func key(_ parts: [String]) -> String { (try! JSONEncoder().encode(parts)).base64EncodedString() }
    func page(limit: Int, offset: Int, search: String, set: String = "") -> InventoryPage {
        let slugs = setSlugs()
        let all = entries.values.map { original in
            var entry = original
            entry.setKey = slugs[original.setKey]!
            return entry
        }
        let query = search.trimmingCharacters(in: .whitespacesAndNewlines)
        let filtered = all.filter {
            (set.isEmpty || $0.setKey == set) && (query.isEmpty || [$0.name, $0.setName ?? "", $0.number ?? "", $0.game.rawValue, $0.language]
                .joined(separator: " ").localizedCaseInsensitiveContains(query))
        }.sorted {
            if $0.quantity != $1.quantity { return $0.quantity > $1.quantity }
            if $0.name != $1.name { return $0.name.localizedStandardCompare($1.name) == .orderedAscending }
            return $0.id < $1.id
        }
        func count(_ status: String) -> Int { all.filter { $0.status == status }.reduce(0) { $0 + $1.quantity } }
        var page = InventoryPage(entries: Array(filtered.dropFirst(offset).prefix(limit)), totalCopies: all.reduce(0) { $0 + $1.quantity },
            catalogueCopies: count("catalogue"), awaitingCatalogueCopies: count("awaitingCatalogue"), unresolvedCopies: count("unresolved"),
            excludedCodeCards: excludedCodeCards, matchingEntries: filtered.count,
            nextOffset: offset + limit < filtered.count ? offset + limit : nil)
        page.sets = Dictionary(grouping: all, by: \.setKey).map { id, entries in
            let sorted = entries.sorted { $0.id < $1.id }
            // Set covers should be actual prepared cards, not entire overscan
            // canvases. Prefer portrait cards for the fan; landscape-only sets
            // retain their real aspect ratio in the web layout.
            let covers = sorted.compactMap(\.preview).filter { $0.editID != nil }.sorted {
                let leftPortrait = $0.image.width <= $0.image.height
                let rightPortrait = $1.image.width <= $1.image.height
                if leftPortrait != rightPortrait { return leftPortrait }
                if $0.batchID != $1.batchID { return $0.batchID.uuidString < $1.batchID.uuidString }
                return $0.image.sequence < $1.image.sequence
            }
            return InventorySet(id: id, name: sorted[0].setName ?? "Unidentified", game: sorted[0].game,
                quantity: sorted.reduce(0) { $0 + $1.quantity }, entries: sorted.count,
                previews: Array(covers.prefix(3)))
        }.sorted { $0.name != $1.name ? $0.name.localizedStandardCompare($1.name) == .orderedAscending : $0.id < $1.id }
        return page
    }
}

extension ScannerCoordinator {
    public func inventory(limit: Int = 50, offset: Int = 0, search: String = "", set: String = "") async throws -> InventoryPage {
        guard (1...100).contains(limit), (0...1_000_000).contains(offset), search.utf8.count <= 160, set.utf8.count <= 512 else {
            throw ScannerError("invalidParams", "Inventory requires limit 1–100, a valid offset and search up to 160 bytes.")
        }
        let builder = try await inventorySnapshot()
        return builder.page(limit: limit, offset: offset, search: search, set: set)
    }
    public func missingInventoryCards(set: String) async throws -> InventoryMissingCards {
        guard !set.isEmpty, set.utf8.count <= 256 else { throw ScannerError("invalidParams", "Select an inventory set.") }
        let builder = try await inventorySnapshot()
        guard let source = try builder.catalogueSource(set: set) else {
            return InventoryMissingCards(note: "A catalogue checklist is not available for this set yet.")
        }
        let cards = try await inventoryCatalogue.cards(in: source)
        return builder.missingCards(cards, source: source)
    }
    public func inventoryReference(key: String) async throws -> ImageThumbnail {
        guard thumbnailWorkers < 2 else { throw ScannerError("resourceLimit", "Previews are busy; retry shortly.") }
        thumbnailWorkers += 1
        defer { thumbnailWorkers -= 1 }
        return try await inventoryCatalogue.thumbnail(key: key)
    }
    private func inventorySnapshot() async throws -> InventoryBuilder {
        guard inventoryWorkers < 4 else { throw ScannerError("busy", "Inventory is busy; retry shortly.") }
        inventoryWorkers += 1
        defer { inventoryWorkers -= 1 }
        let database = database
        let worker = Task.detached(priority: .utility) {
            var builder = InventoryBuilder()
            try database.readInventoryRecords { builder.add($0) }
            return builder
        }
        return try await withTaskCancellationHandler { try await worker.value } onCancel: { worker.cancel() }
    }
}
