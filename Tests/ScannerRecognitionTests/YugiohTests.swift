import Foundation
import CoreGraphics
import ImageIO
import Testing
import ScannerAPI
@testable import ScannerRecognition

private let ygoJSON = Data(#"{"data":[{"id":39706423,"name":"GMX Suppression Squad","type":"Effect Monster","card_sets":[{"set_name":"Blazing Dominion","set_code":"BLZD-EN091","set_rarity":"Common"},{"set_name":"Other","set_code":"UP02-EN008","set_rarity":"Super Rare"}]}]}"#.utf8)
private func ygoLines(name: String = "GMX Suppression Squad", code: String = "BLZD-EN091", passcode: String = "39706423") -> [RecognizedLine] {
    [RecognizedLine(text: name, y: 0.9), RecognizedLine(text: code, y: 0.3), RecognizedLine(text: passcode + " 1st Edition", y: 0.02)]
}
@Test func yugiohMatchingRequiresConsistentIdentityAndKeepsPrintingUncertainty() throws {
    let cards = try YugiohCatalogue.decode(ygoJSON)
    let match = YugiohMatcher.match(lines: ygoLines(), cards: cards)
    let card = try #require(match.candidates.first)
    #expect(card.name == "GMX Suppression Squad" && card.localID == "BLZD-EN091")
    #expect(card.provider == "ygoprodeck" && card.catalogueID == "39706423")
    #expect(card.rarities == ["Common"] && card.availableFinishes.isEmpty)
    #expect(YugiohMatcher.match(lines: ygoLines(passcode: "12345678"), cards: cards).candidates.isEmpty)
    #expect(YugiohMatcher.match(lines: ygoLines(name: "Unreadable", passcode: ""), cards: cards).candidates.isEmpty)
    #expect(YugiohMatcher.match(lines: ygoLines(code: "UNKNOWN-999"), cards: cards).candidates.first?.localID == "")
    #expect(YugiohMatcher.match(lines: ygoLines(passcode: ""), cards: cards).candidates.first?.localID == "BLZD-EN091")
    #expect(YugiohMatcher.match(lines: ygoLines(name: ""), cards: cards).candidates.first?.localID == "BLZD-EN091")
    var multiple = cards
    multiple[0].card_sets?.append(.init(set_name: "Blazing Dominion", set_code: "BLZD-EN091", set_rarity: "Ultra Rare"))
    #expect(YugiohMatcher.match(lines: ygoLines(), cards: multiple).candidates.first?.rarities == ["Common", "Ultra Rare"])
    var lowConfidence = ygoLines(); for index in lowConfidence.indices { lowConfidence[index].confidence = 0.2 }
    #expect(YugiohMatcher.match(lines: lowConfidence, cards: cards).candidates.isEmpty)
    #expect(throws: (any Error).self) { try YugiohCatalogue.decode(Data(#"{"data":[]}"#.utf8)) }
}
private actor YgoFetch {
    var calls = 0
    func get(_ url: URL, limit: Int) async throws -> Data {
        calls += 1
        #expect(url == YugiohCatalogue.endpoint && limit == YugiohCatalogue.catalogueLimit)
        try await Task.sleep(for: .milliseconds(20))
        return ygoJSON
    }
}
@Test func yugiohCatalogueCachesCoalescesAndFallsBackOffline() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let fetch = YgoFetch()
    let client = YugiohCatalogue(root: root, fetch: { try await fetch.get($0, limit: $1) })
    async let one = client.catalogue(); async let two = client.catalogue()
    #expect(try await one.cards.count == 1)
    #expect(try await two.cards.count == 1)
    #expect(await fetch.calls == 1)
    let offline = YugiohCatalogue(root: root, fetch: { _, _ in throw ScannerError("offline", "Fixture") })
    #expect(try await offline.catalogue().stale == false)
    try FileManager.default.setAttributes([.modificationDate: Date().addingTimeInterval(-3 * 86400)], ofItemAtPath: root.appendingPathComponent("catalogue.json").path)
    let stale = YugiohCatalogue(root: root, fetch: { _, _ in throw ScannerError("offline", "Fixture") })
    #expect(try await stale.catalogue().stale)
    let empty = YugiohCatalogue(root: root.appendingPathComponent("empty"), fetch: { _, _ in throw ScannerError("offline", "Fixture") })
    await #expect(throws: ScannerError.self) { try await empty.catalogue() }
}
@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_YUGIOH_BATCH"] != nil))
func savedYugiohFrontsMatchPrintingsUsingLocalOCR() async throws {
    let path = try #require(ProcessInfo.processInfo.environment["SCANNER_YUGIOH_BATCH"])
    let cataloguePath = try #require(ProcessInfo.processInfo.environment["SCANNER_YUGIOH_CATALOGUE"])
    let data = try Data(contentsOf: URL(fileURLWithPath: cataloguePath))
    let record = try Wire.decoder().decode(ScanRecord.self, from: Data(contentsOf: URL(fileURLWithPath: path).appendingPathComponent("manifest.json")))
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let catalogue = YugiohCatalogue(root: root, fetch: { url, _ in
        guard url == YugiohCatalogue.endpoint else { throw ScannerError("test", "Reference download disabled in offline fixture") }
        return data
    })
    let recognizer = YugiohRecognizer(catalogue: catalogue)
    for pair in CardGameDetection.resolve(record) {
        let sequence = try #require(pair.frontSequence)
        let edit = try #require(record.processedImages?.last { $0.request.sequence == sequence })
        let bytes = try Data(contentsOf: URL(fileURLWithPath: path).appendingPathComponent(edit.image.relativePath))
        let result = try await recognizer.recognize(bytes, language: .english)
        print("YGO OCR", sequence, result.lines.map(\.text))
        print("YGO MATCH", sequence, result.candidates.map { ($0.name, $0.localID, $0.rarities ?? []) })
        #expect(result.candidates.count == 1)
        #expect(result.candidates.first?.localID.isEmpty == false)
    }
}

private actor YgoArtworkFetch {
    var calls = 0
    let bytes: Data
    init(bytes: Data) { self.bytes = bytes }
    func get(_ url: URL, limit: Int) -> Data {
        calls += 1
        #expect(url.absoluteString == "https://images.ygoprodeck.com/images/cards/39706423.jpg")
        #expect(limit == YugiohCatalogue.imageLimit)
        return bytes
    }
}
@Test func yugiohReferenceCacheReusesValidatedImagesAndRejectsLinks() async throws {
    let root = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let context = try #require(CGContext(data: nil, width: 200, height: 300, bitsPerComponent: 8, bytesPerRow: 800,
        space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.premultipliedLast.rawValue))
    let data = NSMutableData()
    let output = try #require(CGImageDestinationCreateWithData(data, "public.png" as CFString, 1, nil))
    CGImageDestinationAddImage(output, try #require(context.makeImage()), nil)
    #expect(CGImageDestinationFinalize(output))
    let fetch = YgoArtworkFetch(bytes: data as Data)
    let cache = YugiohCatalogue(root: root, fetch: { await fetch.get($0, limit: $1) })
    var card = try #require(YugiohCatalogue.decode(ygoJSON).first)
    card.card_images = [.init(id: 39706423)]
    try await cache.cacheReference(card)
    try await cache.cacheReference(card)
    #expect(await fetch.calls == 1)
    let image = root.appendingPathComponent("reference-39706423.jpg")
    try Data("not an image".utf8).write(to: image)
    try await cache.cacheReference(card)
    #expect(await fetch.calls == 2)
    try FileManager.default.removeItem(at: image)
    try FileManager.default.createSymbolicLink(at: image, withDestinationURL: root.appendingPathComponent("catalogue.json"))
    try ygoJSON.write(to: root.appendingPathComponent("catalogue.json"))
    await #expect(throws: ScannerError.self) { try await cache.cacheReference(card) }
    #expect(!YugiohCatalogue.validImage(Data("<html>error</html>".utf8)))
}
