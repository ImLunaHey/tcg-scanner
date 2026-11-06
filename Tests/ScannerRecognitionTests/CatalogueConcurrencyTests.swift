import Foundation
import Testing
@testable import ScannerRecognition

private actor CatalogueRequests {
    var count = 0
    func load() async throws -> Data {
        count += 1
        try await Task.sleep(for: .milliseconds(30))
        return Data(#"[{"id":"fixture-1","localId":"1","name":"Fixture"}]"#.utf8)
    }
}
@Test func parallelCardWorkersShareOneCatalogueDownload() async throws {
    let requests = CatalogueRequests()
    let client = TCGdexClient(load: { _,_ in try await requests.load() })
    try await withThrowingTaskGroup(of: Int.self) { group in
        for _ in 0..<8 { group.addTask { try await client.catalogue().count } }
        for try await count in group { #expect(count == 1) }
    }
    #expect(await requests.count == 1)
    #expect(try await client.catalogue().count == 1)
    #expect(await requests.count == 1)
}
