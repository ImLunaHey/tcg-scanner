import CoreGraphics
import CryptoKit
import Foundation
import ImageIO
import Testing
@testable import ScannerRecognition

private func referencePNG(width: Int = 32, height: Int = 44) throws -> Data {
    let pixels = Data(repeating: 128, count: width * height * 4)
    let provider = try #require(CGDataProvider(data: pixels as CFData))
    let colourSpace = try #require(CGColorSpace(name: CGColorSpace.sRGB))
    let image = try #require(CGImage(width: width, height: height, bitsPerComponent: 8, bitsPerPixel: 32,
        bytesPerRow: width * 4, space: colourSpace,
        bitmapInfo: CGBitmapInfo.byteOrder32Big.union(CGBitmapInfo(rawValue: CGImageAlphaInfo.premultipliedLast.rawValue)),
        provider: provider, decode: nil, shouldInterpolate: false, intent: .defaultIntent))
    let bytes = NSMutableData()
    let destination = try #require(CGImageDestinationCreateWithData(bytes, "public.png" as CFString, 1, nil))
    CGImageDestinationAddImage(destination, image, nil)
    #expect(CGImageDestinationFinalize(destination))
    return bytes as Data
}

private actor ReferenceFixture {
    var metadataCalls = 0
    var downloads = 0
    let data: Data
    let base: String?
    let status: Int
    let mime: String?
    let redirect: URL?
    init(data: Data, base: String? = "https://assets.tcgdex.net/en/sv/sv01/104",
         status: Int = 200, mime: String? = "image/png", redirect: URL? = nil) {
        self.data = data; self.base = base; self.status = status; self.mime = mime; self.redirect = redirect
    }
    func metadata(_ id: String) async throws -> String? { metadataCalls += 1; await Task.yield(); return base }
    func fetch(_ url: URL) async throws -> TCGdexReferenceCache.Download {
        downloads += 1; await Task.yield()
        return .init(data: data, responseURL: redirect ?? url, statusCode: status, mimeType: mime)
    }
    var calls: (Int, Int) { (metadataCalls, downloads) }
}

private func referenceRoot() -> URL { FileManager.default.temporaryDirectory.appendingPathComponent("scanner-reference-" + UUID().uuidString) }
private func cache(_ root: URL, _ fixture: ReferenceFixture, maximumBytes: Int = 256 * 1024 * 1024,
                   date: Date = Date(timeIntervalSince1970: 1_800_000_000)) -> TCGdexReferenceCache {
    TCGdexReferenceCache(root: root, maximumBytes: maximumBytes, now: { date },
        metadata: { try await fixture.metadata($0) }, download: { try await fixture.fetch($0) })
}

@Test func referenceCacheUsesValidatedPersistentHitsAndCoalescesDownloads() async throws {
    let root = referenceRoot(); defer { try? FileManager.default.removeItem(at: root) }
    let fixture = ReferenceFixture(data: try referencePNG()), provider = cache(root, fixture)
    let responses = try await withThrowingTaskGroup(of: CardReferenceImage?.self) { group in
        for _ in 0..<8 { group.addTask { try await provider.reference(cardID: "sv01-104") } }
        var responses: [CardReferenceImage] = []
        for try await value in group { responses.append(try #require(value)) }
        return responses
    }
    #expect(responses.count == 8)
    #expect(Set(responses.map(\.sha256)).count == 1)
    let count = await fixture.calls; #expect(count.0 == 1 && count.1 == 1)
    let restarted = cache(root, fixture)
    let diskHit = try #require(try await restarted.reference(cardID: "sv01-104"))
    #expect(diskHit.cacheHit && diskHit.data == responses[0].data)
    #expect(diskHit.url.absoluteString == "https://assets.tcgdex.net/en/sv/sv01/104/high.png")
    let after = await fixture.calls; #expect(after.0 == 1 && after.1 == 1)
}

@Test func referenceCacheRejectsUnsafeAssetURLsAndIdentifiers() async throws {
    for base in [
        "http://assets.tcgdex.net/en/sv/sv01/104", "https://evil.example/en/sv/sv01/104",
        "https://assets.tcgdex.net.evil.example/en/sv/sv01/104", "https://user@assets.tcgdex.net/en/sv/sv01/104",
        "https://assets.tcgdex.net:443/en/sv/sv01/104", "https://assets.tcgdex.net/en/sv/sv01/104?x=1",
        "https://assets.tcgdex.net/en/sv/sv01/104#x", "https://assets.tcgdex.net/en/../sv01/104",
        "https://assets.tcgdex.net/en/%2e%2e/sv01/104", "https://assets.tcgdex.net/en/sv/sv01/104/extra",
        "https://assets.tcgdex.net//en/sv/sv01/104", "https://assets.tcgdex.net/en/sv/sv01/104/"
    ] { #expect(throws: (any Error).self) { try TCGdexReferenceCache.assetURL(base) } }
    let root = referenceRoot(); defer { try? FileManager.default.removeItem(at: root) }
    let fixture = ReferenceFixture(data: try referencePNG()), provider = cache(root, fixture)
    for card in ["", "..", "../sv01-104", "/tmp/card", "sv01%2f104"] {
        await #expect(throws: (any Error).self) { try await provider.reference(cardID: card) }
    }
    let calls = await fixture.calls; #expect(calls.0 == 0 && calls.1 == 0)
}

@Test func referenceCacheNeverTreatsRedirectsInvalidOrOversizedDataAsReferences() async throws {
    let valid = try referencePNG()
    let cases = [
        ReferenceFixture(data: valid, redirect: URL(string: "https://evil.example/card.png")!),
        ReferenceFixture(data: valid, status: 302), ReferenceFixture(data: valid, mime: "text/html"),
        ReferenceFixture(data: Data([1, 2, 3])),
        ReferenceFixture(data: Data(repeating: 0, count: 8 * 1024 * 1024 + 1)),
        ReferenceFixture(data: try referencePNG(width: 20_001, height: 1))
    ]
    for fixture in cases {
        let root = referenceRoot(); defer { try? FileManager.default.removeItem(at: root) }
        let provider = cache(root, fixture)
        await #expect(throws: (any Error).self) { try await provider.reference(cardID: "sv01-104") }
        #expect(!FileManager.default.fileExists(atPath: root.path))
    }
}

@Test func referenceCacheDistinguishesMissingImagesFromFailedDownloads() async throws {
    for fixture in [ReferenceFixture(data: Data(), base: nil), ReferenceFixture(data: Data(), status: 404)] {
        let root = referenceRoot(); defer { try? FileManager.default.removeItem(at: root) }
        let provider = cache(root, fixture)
        #expect(try await provider.reference(cardID: "sve-010") == nil)
        #expect(try await provider.reference(cardID: "sve-010") == nil)
        let calls = await fixture.calls; #expect(calls.0 == 1 && calls.1 <= 1)
    }
    let root = referenceRoot(); defer { try? FileManager.default.removeItem(at: root) }
    let failed = cache(root, ReferenceFixture(data: Data(), status: 503))
    await #expect(throws: (any Error).self) { try await failed.reference(cardID: "sv01-104") }
}

@Test func referenceCacheRevalidatesExpiredOrCorruptEntries() async throws {
    let root = referenceRoot(); defer { try? FileManager.default.removeItem(at: root) }
    let fixture = ReferenceFixture(data: try referencePNG())
    let first = cache(root, fixture)
    _ = try await first.reference(cardID: "sv01-104")
    let png = try #require(try FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: nil).first { $0.pathExtension == "png" })
    try Data([9, 8, 7]).write(to: png)
    let replacement = try #require(try await cache(root, fixture).reference(cardID: "sv01-104"))
    #expect(!replacement.cacheHit)
    let expired = cache(root, fixture, date: Date(timeIntervalSince1970: 1_800_000_000 + 8 * 86400))
    #expect(try #require(try await expired.reference(cardID: "sv01-104")).cacheHit == false)
    let calls = await fixture.calls; #expect(calls.0 == 3 && calls.1 == 3)
}

@Test func referenceCacheBoundsDiskUsageAndPrunesInterruptedWrites() async throws {
    let root = referenceRoot(); defer { try? FileManager.default.removeItem(at: root) }
    try FileManager.default.createDirectory(at: root, withIntermediateDirectories: true)
    let orphan = root.appendingPathComponent(String(repeating: "a", count: 64) + ".png")
    try Data(repeating: 0, count: 2000).write(to: orphan)
    let png = try referencePNG(), fixture = ReferenceFixture(data: png)
    let budget = png.count + 450, provider = cache(root, fixture, maximumBytes: budget)
    _ = try await provider.reference(cardID: "sv01-104")
    _ = try await provider.reference(cardID: "sv02-097")
    #expect(!FileManager.default.fileExists(atPath: orphan.path))
    let files = try FileManager.default.contentsOfDirectory(at: root, includingPropertiesForKeys: [.fileSizeKey])
    let bytes = try files.reduce(0) { try $0 + ($1.resourceValues(forKeys: [.fileSizeKey]).fileSize ?? 0) }
    #expect(bytes <= budget)
    #expect(files.filter { $0.pathExtension == "png" }.count == 1)
    let tooSmall = referenceRoot(); defer { try? FileManager.default.removeItem(at: tooSmall) }
    let memoryOnly = try #require(try await cache(tooSmall, fixture, maximumBytes: 1).reference(cardID: "sv01-104"))
    #expect(!memoryOnly.cacheHit && !FileManager.default.fileExists(atPath: tooSmall.path))
}

@Test func referenceCacheRejectsSymlinkRootsWithoutTouchingTargets() async throws {
    let root = referenceRoot(), target = referenceRoot()
    defer { try? FileManager.default.removeItem(at: root); try? FileManager.default.removeItem(at: target) }
    try FileManager.default.createDirectory(at: target, withIntermediateDirectories: true)
    try FileManager.default.createSymbolicLink(at: root, withDestinationURL: target)
    let fixture = ReferenceFixture(data: try referencePNG()), provider = cache(root, fixture)
    await #expect(throws: (any Error).self) { try await provider.reference(cardID: "sv01-104") }
    #expect(try FileManager.default.contentsOfDirectory(atPath: target.path).isEmpty)
    let calls = await fixture.calls; #expect(calls.0 == 0 && calls.1 == 0)
}
