import Foundation
import Testing
@testable import ScannerAPI

private func gameRecord(_ bases: [String?]) -> ScanRecord {
    var record = ScanRecord(device: Device(id: "mock", name: "Mock"), request: ScanRequest(duplex: true), outputDirectory: "/unused")
    record.processedImages = []
    for (index, basis) in bases.enumerated() {
        let image = OriginalImage(sequence: index + 1, relativePath: "image.png", bytes: 1, sha256: "source-\(index)", width: 600, height: 840, imageCount: 1, uniformType: "public.png")
        record.images.append(image)
        var edit = ProcessedImage(request: ImageEditRequest(jobID: record.id, sequence: image.sequence), sourceSHA256: image.sha256, image: image)
        edit.cardPreparation = CardPreparation(crop: ImageCrop(), quarterTurns: 0, orientationVerified: basis != nil, warnings: [], orientationBasis: basis)
        record.processedImages?.append(edit)
    }
    return record
}

@Test func mixedGamesResolvePerPairInEitherFeedDirection() throws {
    let record = gameRecord(["pokemon-back-pattern", "text", "text", "yugioh-back-pattern", "text", "text"])
    let pairs = CardGameDetection.resolve(record)
    #expect(pairs.map(\.game) == [.pokemon, .yugioh, .unknown])
    #expect(pairs[0].frontSequence == 2 && pairs[0].backSequence == 1)
    #expect(pairs[1].frontSequence == 3 && pairs[1].backSequence == 4)
    #expect(pairs[2].frontSequence == nil)
    #expect(record.detectedGame(for: 4) == .yugioh)
}

@Test func unknownAmbiguousIncompleteAndStaleBacksNeverDefaultToPokemon() {
    for bases: [String?] in [[nil, "text"], ["pokemon-back-pattern", "yugioh-back-pattern"], ["pokemon-back-pattern", "pokemon-back-pattern"], ["yugioh-back-pattern"]] {
        #expect(CardGameDetection.resolve(gameRecord(bases)).allSatisfy { $0.game == .unknown })
    }
    var record = gameRecord(["pokemon-back-pattern", "text"])
    record.processedImages?[0].sourceSHA256 = "different-original"
    #expect(record.detectedGame(for: 1) == .unknown)
    record = gameRecord(["pokemon-back-pattern", "text"])
    record.request.duplex = false
    #expect(record.detectedGame(for: 1) == .unknown)
    record = gameRecord(["pokemon-back-pattern", "text", "yugioh-back-pattern", "text"])
    record.images.remove(at: 1)
    let pairs = CardGameDetection.resolve(record)
    #expect(pairs[0].game == .unknown && pairs[1].game == .yugioh)
}

@Test func onlineCodeCardsNeedTwoCurrentSidesAndNeverBecomePlayableCards() {
    var record = gameRecord(["pokemon-code-text", "pokemon-code-text"])
    let pair = CardGameDetection.resolve(record)[0]
    #expect(pair.contentKind == .pokemonCodeCard)
    #expect(pair.game == .unknown && pair.frontSequence == nil)
    #expect(record.detectedGame(for: 1) == .unknown)
    #expect(CardGameDetection.resolve(gameRecord(["pokemon-code-text", "text"]))[0].contentKind == nil)
    #expect(CardGameDetection.resolve(gameRecord(["pokemon-code-text"]))[0].contentKind == nil)
    record.processedImages?[0].sourceSHA256 = "stale"
    #expect(CardGameDetection.resolve(record)[0].contentKind == nil)
}

@Test func oneCurrentRedemptionURLIdentifiesTheCompleteCodeCardPair() {
    for bases: [String?] in [["unknown", "pokemon-code-url"], ["pokemon-code-url", nil], ["pokemon-code-text", "pokemon-code-url"]] {
        let result = CardGameDetection.resolve(gameRecord(bases))[0]
        #expect(result.contentKind == .pokemonCodeCard && result.game == .unknown)
        #expect(result.frontSequence == nil && result.backSequence == nil)
    }
    #expect(CardGameDetection.resolve(gameRecord(["pokemon-code-url"]))[0].contentKind == nil)
    var stale = gameRecord(["unknown", "pokemon-code-url"])
    stale.processedImages?[1].sourceSHA256 = "stale"
    #expect(CardGameDetection.resolve(stale)[0].contentKind == nil)
    for back in ["pokemon-back-pattern", "yugioh-back-pattern"] {
        let conflict = CardGameDetection.resolve(gameRecord([back, "pokemon-code-url"]))[0]
        #expect(conflict.contentKind == nil && conflict.game == .unknown)
        #expect(conflict.reason.contains("conflict"))
    }
}

@Test func starWarsPairsUseLogoBackOrRetainTwoNumberedFaces() {
    let standard = CardGameDetection.resolve(gameRecord(["starwars-back-pattern", "text"]))[0]
    #expect(standard.game == .starwarsUnlimited && standard.frontSequence == 2 && standard.backSequence == 1)
    let dual = CardGameDetection.resolve(gameRecord(["text", "starwars-card-text"]))[0]
    #expect(dual.game == .starwarsUnlimited && dual.frontSequence == nil && dual.backSequence == nil)
    #expect(CardGameDetection.resolve(gameRecord(["starwars-back-pattern", "pokemon-back-pattern"]))[0].game == .unknown)
    #expect(CardGameDetection.resolve(gameRecord(["starwars-back-pattern", "pokemon-code-url"]))[0].game == .unknown)
}
