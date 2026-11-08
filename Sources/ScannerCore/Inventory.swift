import Foundation
import ScannerAPI

/// Counts acquisition pairs, never image edits or OCR attempts. Rescans in a
/// different batch intentionally remain additional copies.
struct InventoryBuilder {
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
                entry = InventoryEntry(id: id, game: game, language: language, name: card.name, setName: card.setName,
                    number: card.localID + (card.officialSetCount.map { "/\($0)" } ?? ""), status: "catalogue", copy: copy, confirmed: identity.state == .confirmed)
            } else if complete, game != .unknown, matched.isEmpty, waiting.count == 1, let printed = waiting.first?.evidence?.uncataloguedCard {
                let id = key(base + ["awaitingCatalogue", printed.setCode, printed.collectorNumber, printed.name.lowercased()])
                entry = InventoryEntry(id: id, game: game, language: language, name: printed.name,
                    setName: printed.setName ?? printed.setCode, number: printed.collectorNumber, status: "awaitingCatalogue", copy: copy, confirmed: false)
            } else {
                let id = key(["unresolved", record.id.uuidString, String(number)])
                entry = InventoryEntry(id: id, game: game, language: language,
                    name: complete ? "Needs identification" : "Incomplete card pair", setName: nil, number: nil,
                    status: "unresolved", copy: copy, confirmed: false)
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
    private func key(_ parts: [String]) -> String { (try! JSONEncoder().encode(parts)).base64EncodedString() }
    func page(limit: Int, offset: Int, search: String, set: String = "") -> InventoryPage {
        let all = Array(entries.values)
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
        guard inventoryWorkers < 4 else { throw ScannerError("busy", "Inventory is busy; retry shortly.") }
        inventoryWorkers += 1
        defer { inventoryWorkers -= 1 }
        let database = database
        let worker = Task.detached(priority: .utility) {
            var builder = InventoryBuilder()
            try database.readInventoryRecords { builder.add($0) }
            return builder.page(limit: limit, offset: offset, search: search, set: set)
        }
        return try await withTaskCancellationHandler { try await worker.value } onCancel: { worker.cancel() }
    }
}
