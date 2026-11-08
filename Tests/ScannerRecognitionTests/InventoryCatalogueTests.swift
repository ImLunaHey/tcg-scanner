import CoreGraphics
import Foundation
import ImageIO
import Testing
import ScannerAPI
@testable import ScannerRecognition

private actor GalleryCalls {
    var count = 0
    func read(_ url: URL) async throws -> Data {
        count += 1
        #expect(url.path == "/v2/en/sets/test")
        try await Task.sleep(for: .milliseconds(10))
        return Data(#"{"id":"test","cards":[{"id":"test-001","localId":"001","name":"One","image":"https://assets.tcgdex.net/en/sv/test/001"}]}"#.utf8)
    }
}
@Test func inventoryCatalogueCoalescesSetListsAndRejectsForeignURLs() async throws {
    let calls = GalleryCalls()
    let provider = InventoryCatalogue(pokemon: [.english: TCGdexClient(load: { url,_ in try await calls.read(url) })],
        yugioh: YugiohCatalogue(), starwars: StarWarsCatalogue(), thumbnails: CatalogueThumbnails())
    let source = InventoryCatalogueSource(provider: "tcgdex", setID: "test", language: .english)
    async let first = provider.cards(in: source)
    async let second = provider.cards(in: source)
    #expect(try await first.first?.thumbnailKey?.count == 64)
    #expect(try await second.first?.id == "test-001")
    #expect(await calls.count == 1)
    for address in ["http://assets.tcgdex.net/en/sv/test/001/low.png", "https://evil.test/image.png",
                    "https://assets.tcgdex.net/en/../test/001/low.png", "https://assets.tcgdex.net/en/sv/test/001/low.png?redirect=1"] {
        #expect(!CatalogueThumbnails.allowed(URL(string: address)!))
    }
}
@Test func inventoryCatalogueThumbnailPersistsAndRejectsArbitraryKeys() async throws {
    let root = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let context = try #require(CGContext(data: nil, width: 100, height: 140, bitsPerComponent: 8, bytesPerRow: 400,
        space: CGColorSpaceCreateDeviceRGB(), bitmapInfo: CGImageAlphaInfo.noneSkipLast.rawValue))
    let encoded = NSMutableData()
    let dest = try #require(CGImageDestinationCreateWithData(encoded, "public.png" as CFString, 1, nil))
    CGImageDestinationAddImage(dest, try #require(context.makeImage()), nil)
    #expect(CGImageDestinationFinalize(dest))
    let bytes = encoded as Data
    let cache = CatalogueThumbnails(root: root, fetch: { _ in bytes })
    let key = try #require(await cache.register(URL(string: "https://assets.tcgdex.net/en/sv/test/001/low.png")))
    let one = try await cache.thumbnail(key: key)
    let offline = CatalogueThumbnails(root: root, fetch: { _ in throw ScannerError("test", "No network expected") })
    let two = try await offline.thumbnail(key: key)
    #expect(one.sha256 == two.sha256 && two.mediaType == "image/jpeg" && two.height == 140)
    await #expect(throws: ScannerError.self) { try await offline.thumbnail(key: "../private") }
    await #expect(throws: ScannerError.self) { try await offline.thumbnail(key: String(repeating: "a", count: 64)) }
}
