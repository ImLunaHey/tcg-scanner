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
