import Foundation
import Testing
import ScannerAPI
@testable import ScannerRecognition

private actor DateCalls {
    var count = 0
    func fetch(_ source: InventoryCatalogueSource) async throws -> String? {
        count += 1
        try await Task.sleep(for: .milliseconds(30))
        return "2026-11-07"
    }
}
@Test func inventoryReleaseDatesNormalizeProviderFormats() {
    #expect(InventoryReleaseDates.normalized("2026-11-07") == "2026-11-07")
    #expect(InventoryReleaseDates.normalized("10/9/26") == "2026-10-09")
    #expect(InventoryReleaseDates.normalized("2026-02-31") == nil)
    #expect(InventoryReleaseDates.normalized("unknown") == nil)
}
@Test func inventoryReleaseDatesReturnImmediatelyAndPersist() async throws {
    let root = FileManager.default.temporaryDirectory.resolvingSymlinksInPath().appendingPathComponent(UUID().uuidString)
    defer { try? FileManager.default.removeItem(at: root) }
    let file = root.appendingPathComponent("dates.json"), calls = DateCalls()
    let provider = InventoryReleaseDates(file: file, fetch: { try await calls.fetch($0) })
    let source = InventoryCatalogueSource(provider: "tcgdex", setID: "set", language: .english)
    let first = await provider.snapshot([source])
    #expect(first.pending && first.dates.isEmpty)
    var result = first
    for _ in 0..<100 where result.pending {
        try await Task.sleep(for: .milliseconds(10)); result = await provider.snapshot([source])
    }
    #expect(!result.pending && result.dates[source] == "2026-11-07")
    #expect(await calls.count == 1)
    let offline = InventoryReleaseDates(file: file, fetch: { _ in throw ScannerError("offline", "Unexpected network") })
    let cached = await offline.snapshot([source])
    #expect(!cached.pending && cached.dates[source] == "2026-11-07")
}
