import CryptoKit
import Foundation
import Testing
import ScannerAPI
@testable import ScannerRecognition

/// Explicit, read-only experiment. Fixtures/results remain outside source control.
/// Never talks to ScannerCoordinator, scan storage, hardware or inventory.
@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_BENCH_INPUT"] != nil))
func benchmarkSavedRecognition() async throws {
    struct Item: Decodable { var id: String; var path: String; var sha256: String }
    struct Reading: Decodable { var id: String; var variants: [String: [RecognizedLine]] }
    struct Result: Encodable {
        var id: String
        var evidence: RecognitionEvidence?
        var variants: [String: [String]]?
        var seconds: Double
    }
    let env = ProcessInfo.processInfo.environment
    let input = URL(fileURLWithPath: try #require(env["SCANNER_BENCH_INPUT"]))
    let output = URL(fileURLWithPath: try #require(env["SCANNER_BENCH_OUTPUT"]))
    let items = try JSONDecoder().decode([Item].self, from: Data(contentsOf: input))
    let recognizer = CardRecognizer()
    let cacheRoot = output.deletingLastPathComponent()
    let catalogue = TCGdexClient(load: { url, limit in
        let file = url.path.hasSuffix("/cards") ? cacheRoot.appendingPathComponent("catalogue.json")
            : cacheRoot.appendingPathComponent("metadata-" + url.lastPathComponent + ".json")
        if let cached = try? Data(contentsOf: file) { return cached }
        for attempt in 0..<3 {
            let (data, response) = try await URLSession.shared.data(from: url)
            if (response as? HTTPURLResponse)?.statusCode == 200, !data.isEmpty, data.count <= limit {
                try data.write(to: file, options: .atomic)
                return data
            }
            try await Task.sleep(for: .milliseconds(250 * (attempt + 1)))
        }
        throw ScannerError("benchmark", "Could not snapshot catalogue metadata for \(url.lastPathComponent)")
    })
    let imported: [Reading] = try env["SCANNER_BENCH_OCR"].map {
        try JSONDecoder().decode([Reading].self, from: Data(contentsOf: URL(fileURLWithPath: $0)))
    } ?? []
    let cards = imported.isEmpty ? [] : try await catalogue.catalogue()
    var results: [Result] = []
    for item in items {
        let data = try Data(contentsOf: URL(fileURLWithPath: item.path))
        let digest = SHA256.hash(data: data).map { String(format: "%02x", $0) }.joined()
        try #require(digest == item.sha256, "Fixture changed: \(item.id)")
        let start = Date()
        if imported.isEmpty {
            // Same verified-upright fast path as camera recognition, with the
            // production four-orientation fallback when evidence is insufficient.
            let evidence = try await recognizer.recognize(data, language: .english, preferredOrientation: 0)
            results.append(Result(id: item.id, evidence: evidence, seconds: Date().timeIntervalSince(start)))
            print("BENCH", item.id, evidence.candidates.map(\.id), results.last!.seconds)
        } else {
            let reading = try #require(imported.first { $0.id == item.id })
            var variants: [String: [String]] = [:]
            for key in reading.variants.keys.sorted() {
                let ranks = CardMatcher.rank(lines: reading.variants[key]!, catalogue: cards)
                var ids: [String] = []
                for rank in ranks.prefix(8) {
                    let detail = try await catalogue.detail(rank.card.id)
                    if !rank.totals.isEmpty, let total = detail.set.cardCount?.official,
                       !rank.totals.contains(total) { continue }
                    ids.append(detail.id)
                    if ids.count == 5 { break }
                }
                variants[key] = ids
            }
            results.append(Result(id: item.id, variants: variants, seconds: Date().timeIntervalSince(start)))
            print("BENCH MATCH", item.id, variants)
        }
        let encoder = JSONEncoder(); encoder.outputFormatting = [.prettyPrinted, .sortedKeys]
        try encoder.encode(results).write(to: output, options: .atomic)
    }
}
