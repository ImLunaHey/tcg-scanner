import Foundation
import ScannerAPI

struct CatalogueCard: Decodable, Sendable {
    var id: String
    var localId: String
    var name: String
    var image: String?
}
struct CatalogueDetail: Decodable, Sendable {
    struct CardSet: Decodable, Sendable {
        struct Counts: Decodable, Sendable { var official: Int? }
        var id: String; var name: String; var cardCount: Counts?
    }
    var id: String; var localId: String; var name: String; var set: CardSet
    var variants: [String: Bool]?
}

/// Only catalogue GETs to the fixed TCGdex HTTPS host. No scan or OCR text is uploaded.
actor TCGdexClient {
    private var cache: (Date, [CatalogueCard])?
    private let session: URLSession
    init() {
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = 12
        configuration.timeoutIntervalForResource = 20
        configuration.httpCookieStorage = nil
        configuration.urlCache = nil
        configuration.httpMaximumConnectionsPerHost = 2
        session = URLSession(configuration: configuration, delegate: NoRedirects(), delegateQueue: nil)
    }
    func catalogue() async throws -> [CatalogueCard] {
        if let cache, Date().timeIntervalSince(cache.0) < 24 * 3600 { return cache.1 }
        let data = try await get(path: "cards", limit: 12 * 1024 * 1024)
        let cards = try JSONDecoder().decode([CatalogueCard].self, from: data)
        guard cards.count <= 100_000 else { throw ScannerError("catalogue", "TCGdex catalogue exceeds the supported limit.") }
        // TCG Pocket cards are digital cards, not physical inventory.
        let physical = cards.filter { !($0.image?.contains("/tcgp/") ?? false) }
        cache = (Date(), physical)
        return physical
    }
    func detail(_ id: String) async throws -> CatalogueDetail {
        guard !id.isEmpty, id.utf8.count <= 100, id.utf8.allSatisfy({ (48...57).contains($0) || (65...90).contains($0) || (97...122).contains($0) || $0 == 45 || $0 == 46 }) else {
            throw ScannerError("catalogue", "Invalid catalogue card identifier.")
        }
        return try JSONDecoder().decode(CatalogueDetail.self, from: await get(path: "cards/" + id, limit: 256 * 1024))
    }
    private func get(path: String, limit: Int) async throws -> Data {
        let url = URL(string: "https://api.tcgdex.net/v2/en/" + path)!
        var request = URLRequest(url: url)
        request.setValue("application/json", forHTTPHeaderField: "Accept")
        request.setValue("Scanner/1 (local card identification)", forHTTPHeaderField: "User-Agent")
        let (stream, response) = try await session.bytes(for: request)
        guard let http = response as? HTTPURLResponse, http.statusCode == 200,
              http.url == url, response.expectedContentLength <= limit else {
            throw ScannerError("catalogueUnavailable", "TCGdex returned an unsuccessful or oversized response. Retry identification later.")
        }
        var data = Data()
        for try await byte in stream {
            guard data.count < limit else { throw ScannerError("catalogue", "TCGdex response exceeds the size limit.") }
            data.append(byte)
        }
        return data
    }
}

private final class NoRedirects: NSObject, URLSessionTaskDelegate, Sendable {
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest) async -> URLRequest? { nil }
}
