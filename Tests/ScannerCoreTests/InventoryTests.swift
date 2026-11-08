import Foundation
import Testing
import ScannerAPI
@testable import ScannerCore

private func inventoryRecord(batch: Int = 1, language: CardLanguage = .english, waiting: Bool = false) -> ScanRecord {
    var record = ScanRecord(device: Device(id: "mock", name: "Mock"), request: ScanRequest(duplex: true, prepareCards: true, cardLanguage: language), outputDirectory: "/unused")
    record.state = .completed; record.batchNumber = batch; record.processedImages = []
    for sequence in 1...2 {
        let image = OriginalImage(sequence: sequence, relativePath: "\(sequence).png", bytes: 1, sha256: "hash", width: 200, height: 280, imageCount: 1, uniformType: "public.png")
        record.images.append(image)
        var prepared = ProcessedImage(request: ImageEditRequest(jobID: record.id, sequence: sequence), sourceSHA256: image.sha256, image: image)
        prepared.cardPreparation = CardPreparation(crop: ImageCrop(x: 0, y: 0, width: 1, height: 1), quarterTurns: 0, orientationVerified: true, warnings: [], orientationBasis: sequence == 1 ? "pokemon-back-pattern" : "text")
        record.processedImages?.append(prepared)
    }
    var identity = CardIdentification(request: IdentificationRequest(jobID: record.id, sequence: 2, language: language.rawValue), sourceSHA256: "hash")
    identity.state = waiting ? .awaitingCatalogue : .reviewRequired
    identity.evidence = RecognitionEvidence(lines: [], candidates: waiting ? [] : [CardCandidate(id: "set-001", name: "Card", localID: "001", setID: "set", setName: "Set", evidence: [])])
    if waiting { identity.evidence?.uncataloguedCard = UncataloguedCard(name: "Kyogre", setCode: "DLR", collectorNumber: "026/103") }
    record.cardIdentifications = [identity]
    return record
}
@Test func inventoryCountsPairsAndRescansWithoutCountingEdits() {
    var builder = InventoryBuilder()
    var first = inventoryRecord()
    let extraEdit = first.processedImages![0]
    first.processedImages?.append(extraEdit)
    builder.add(first); builder.add(inventoryRecord(batch: 2))
    let page = builder.page(limit: 50, offset: 0, search: "")
    #expect(page.totalCopies == 2 && page.catalogueCopies == 2 && page.entries.count == 1)
    #expect(page.entries[0].quantity == 2 && page.entries[0].confirmedQuantity == 0)
    #expect(page.entries[0].copies.map(\.cardNumber) == [1, 1])
    #expect(page.entries[0].preview?.image.sequence == 2)
    #expect(page.entries[0].preview?.editID == first.processedImages?[1].id)
    #expect(page.entries[0].preview?.batchID == first.id)
}
@Test func inventorySeparatesLanguagePendingAndAmbiguousCards() {
    var builder = InventoryBuilder()
    builder.add(inventoryRecord()); builder.add(inventoryRecord(language: .korean))
    builder.add(inventoryRecord(waiting: true)); builder.add(inventoryRecord(batch: 2, waiting: true))
    var ambiguous = inventoryRecord(); ambiguous.cardIdentifications?[0].evidence?.candidates.append(CardCandidate(id: "other", name: "Card", localID: "001", setID: "other", setName: "Other", evidence: []))
    builder.add(ambiguous); builder.add(inventoryRecordMissingSide())
    let page = builder.page(limit: 50, offset: 0, search: "")
    #expect(page.totalCopies == 6 && page.catalogueCopies == 2 && page.awaitingCatalogueCopies == 2 && page.unresolvedCopies == 2)
    #expect(page.entries.count == 5)
    let found = builder.page(limit: 1, offset: 0, search: "delta reign")
    #expect(found.entries.count == 1 && found.entries[0].quantity == 2 && found.nextOffset == nil)
    #expect(builder.page(limit: 1, offset: 0, search: "").nextOffset == 1)
    #expect(builder.page(limit: 50, offset: 100, search: "").entries.isEmpty)
}
private func inventoryRecordMissingSide() -> ScanRecord {
    var record = inventoryRecord(); record.images.removeLast(); return record
}
@Test func inventoryIgnoresDocumentsAndActiveJobsAndSeparatesPrintings() {
    var builder = InventoryBuilder()
    var record = inventoryRecord(); record.request.prepareCards = false; record.processedImages = []
    builder.add(record)
    record = inventoryRecord(); record.state = .processing; builder.add(record)
    #expect(builder.page(limit: 50, offset: 0, search: "").totalCopies == 0)
    builder.add(inventoryRecord())
    record = inventoryRecord(); record.cardIdentifications?[0].evidence?.candidates[0].id = "other-001"
    builder.add(record)
    #expect(builder.page(limit: 50, offset: 0, search: "").entries.count == 2)
}
@Test func inventoryExcludesRedemptionCardsAndKeepsReviewCounts() {
    var builder = InventoryBuilder()
    var code = inventoryRecord()
    for index in 0..<2 { code.processedImages?[index].cardPreparation?.orientationBasis = "pokemon-code-text" }
    builder.add(code)
    var reviewed = inventoryRecord()
    reviewed.cardIdentifications?[0].state = .confirmed
    reviewed.cardIdentifications?[0].selectedCardID = "set-001"
    builder.add(reviewed); builder.add(inventoryRecord())
    let page = builder.page(limit: 50, offset: 0, search: "")
    #expect(page.excludedCodeCards == 1 && page.totalCopies == 2)
    #expect(page.entries.first?.confirmedQuantity == 1)
}

@Test func inventorySetsCoverAllPagesAndFilterExactly() {
    var builder = InventoryBuilder()
    builder.add(inventoryRecord()); builder.add(inventoryRecord(waiting: true))
    builder.add(inventoryRecord(batch: 2, waiting: true))
    let first = builder.page(limit: 1, offset: 0, search: "")
    #expect(first.sets.count == 2 && first.entries.count == 1)
    let delta = first.sets.first { $0.name == "Delta Reign" }!
    #expect(delta.quantity == 2 && delta.entries == 1)
    #expect(delta.id == "delta-reign")
    #expect(first.sets.contains { $0.id == "set" })
    #expect(delta.previews.count == 1 && delta.previews[0].image.sequence == 2)
    let selected = builder.page(limit: 50, offset: 0, search: "", set: delta.id)
    #expect(selected.entries.count == 1 && selected.entries[0].quantity == 2)
    #expect(selected.totalCopies == 3 && selected.sets.count == 2)
    #expect(builder.page(limit: 50, offset: 0, search: "does not exist", set: delta.id).entries.isEmpty)
    #expect(builder.page(limit: 50, offset: 0, search: "", set: "wrong").entries.isEmpty)
}

@Test func inventorySetCoversPreferPreparedPortraitsOverRawOrLandscapeScans() {
    var builder = InventoryBuilder()
    for index in 1...5 {
        var record = inventoryRecord(batch: index)
        record.cardIdentifications?[0].evidence?.candidates[0].id = "card-\(index)"
        if index == 1 { record.processedImages?.removeLast() }
        if index == 2 {
            record.processedImages?[1].image.width = 280
            record.processedImages?[1].image.height = 200
        }
        builder.add(record)
    }
    let page = builder.page(limit: 50, offset: 0, search: "")
    #expect(page.totalCopies == 5)
    #expect(page.sets[0].previews.count == 3)
    #expect(page.sets[0].previews.allSatisfy { $0.editID != nil && $0.image.width < $0.image.height })
}

@Test func inventorySetCoversRetainLandscapeOnlyCardsAndOmitUncroppedCanvases() {
    var landscape = inventoryRecord()
    landscape.processedImages?[1].image.width = 280
    landscape.processedImages?[1].image.height = 200
    var builder = InventoryBuilder(); builder.add(landscape)
    let cover = builder.page(limit: 50, offset: 0, search: "").sets[0].previews.first
    #expect(cover?.image.width == 280 && cover?.image.height == 200)
    var raw = inventoryRecord(); raw.processedImages?.removeLast()
    var rawBuilder = InventoryBuilder(); rawBuilder.add(raw)
    let page = rawBuilder.page(limit: 50, offset: 0, search: "")
    #expect(page.totalCopies == 1 && page.entries[0].preview != nil)
    #expect(page.sets[0].previews.isEmpty)
}

@Test func inventorySetSlugsGroupYugiohPrintingPrefixes() {
    var builder = InventoryBuilder()
    for index in 1...2 {
        var record = inventoryRecord(batch: index)
        record.processedImages?[0].cardPreparation?.orientationBasis = "yugioh-back-pattern"
        record.cardIdentifications?[0].evidence?.candidates[0].id = "ygo-\(index)"
        record.cardIdentifications?[0].evidence?.candidates[0].provider = "ygoprodeck"
        record.cardIdentifications?[0].evidence?.candidates[0].setID = "DUEA-EN00\(index)"
        record.cardIdentifications?[0].evidence?.candidates[0].setName = "Name variant \(index)"
        builder.add(record)
    }
    let page = builder.page(limit: 50, offset: 0, search: "")
    #expect(page.sets.count == 1 && page.sets[0].quantity == 2)
    #expect(page.sets[0].id == "name-variant-1")
}

@Test func inventoryUsesReadableSetSlugsWithoutMergingNameCollisions() {
    var builder = InventoryBuilder()
    for (index, name) in ["Ascended Heroes", "Pokémon: Special Collection!", "Same Name", "Same Name"].enumerated() {
        var record = inventoryRecord(batch: index + 1)
        record.cardIdentifications?[0].evidence?.candidates[0].id = "card-\(index)"
        record.cardIdentifications?[0].evidence?.candidates[0].setID = "set-\(index)"
        record.cardIdentifications?[0].evidence?.candidates[0].setName = name
        builder.add(record)
    }
    let page = builder.page(limit: 50, offset: 0, search: "")
    #expect(Set(page.sets.map(\.id)) == ["ascended-heroes", "pokemon-special-collection", "same-name-pokemon", "same-name-pokemon-2"])
    for set in page.sets {
        let selected = builder.page(limit: 50, offset: 0, search: "", set: set.id)
        #expect(selected.entries.count == 1 && selected.entries[0].setKey == set.id)
    }
    #expect(builder.page(limit: 50, offset: 0, search: "", set: "pokemon-tcgdex-set-0").entries.isEmpty)
}
