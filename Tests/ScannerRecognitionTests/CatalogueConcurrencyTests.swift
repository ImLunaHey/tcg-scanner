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

private actor DetailRequests {
    var counts: [String: Int] = [:]
    func load(_ id: String) async throws -> Data {
        counts[id, default: 0] += 1
        if id == "fixture-1" { try await Task.sleep(for: .milliseconds(20)) }
        if id == "retry-1", counts[id] == 1 { throw NSError(domain: "fixture", code: 1) }
        return try JSONSerialization.data(withJSONObject: ["id": id, "localId": "1", "name": "Fixture", "set": ["id": "fixture", "name": "Fixture"]])
    }
}
@Test func repeatedAndConcurrentCardDetailsUseBoundedMetadataCache() async throws {
    let requests = DetailRequests()
    let client = TCGdexClient(load: { url, _ in try await requests.load(url.lastPathComponent) })
    try await withThrowingTaskGroup(of: String.self) { group in
        for _ in 0..<8 { group.addTask { try await client.detail("fixture-1").id } }
        for try await id in group { #expect(id == "fixture-1") }
    }
    _ = try await client.detail("fixture-1")
    #expect(await requests.counts["fixture-1"] == 1)
    for index in 2...130 { _ = try await client.detail("fixture-\(index)") }
    _ = try await client.detail("fixture-1")
    #expect(await requests.counts["fixture-1"] == 2)
    await #expect(throws: (any Error).self) { try await client.detail("retry-1") }
    #expect(try await client.detail("retry-1").id == "retry-1")
    #expect(await requests.counts["retry-1"] == 2)
}
