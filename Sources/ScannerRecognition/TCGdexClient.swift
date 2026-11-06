import Foundation
import ScannerAPI

struct CatalogueCard: Decodable, Sendable {
    var id: String
    var localId: String
    var name: String
    var image: String?
}
struct CatalogueDetail: Decodable, Sendable {
    struct Variant: Decodable, Sendable {
        var variantId: String?
        var type: String
        var size: String?
        var stamp: [String]?
        var subtype: String?
    }
    struct CardSet: Decodable, Sendable {
        struct Counts: Decodable, Sendable { var official: Int? }
        var id: String; var name: String; var cardCount: Counts?
    }
    var id: String; var localId: String; var name: String; var set: CardSet
    var image: String?
    var variants: [String: Bool]?
    var rarity: String?
    var category: String?
    var types: [String]?
    var variants_detailed: [Variant]?
    var printingVariants: [CardPrintingVariant] {
        (variants_detailed ?? []).compactMap { variant in
            guard let id = variant.variantId, variant.size == "standard", variant.subtype == nil,
                  let finish: CardFinish = ["normal": .normal, "holo": .holo, "reverse": .reverseHolo][variant.type] else { return nil }
            return CardPrintingVariant(id: id, finish: finish, stamps: variant.stamp ?? [])
        }
    }
}

/// Only catalogue GETs to the fixed TCGdex HTTPS host. No scan or OCR text is uploaded.
actor TCGdexClient {
    private var cache: (Date, [CatalogueCard])?
    private let session: URLSession
    private let language: CardLanguage
    private let load: (@Sendable (URL, Int) async throws -> Data)?
    init(language: CardLanguage = .english, load: (@Sendable (URL, Int) async throws -> Data)? = nil) {
        self.language = language; self.load = load
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
        let physical = cards.filter {
            !$0.id.isEmpty && $0.id.utf8.count <= 100 && $0.name.utf8.count <= 128 && $0.localId.utf8.count <= 32
                && !($0.image?.contains("/tcgp/") ?? false)
        }
        cache = (Date(), physical)
        return physical
    }
    func detail(_ id: String) async throws -> CatalogueDetail {
        try validate(id)
        let detail = try JSONDecoder().decode(CatalogueDetail.self, from: await get(path: "cards/" + id, limit: 256 * 1024))
        guard detail.id == id, detail.localId.utf8.count <= 32, detail.name.utf8.count <= 128,
              detail.set.id.utf8.count <= 100, detail.set.name.utf8.count <= 256,
              (detail.category?.utf8.count ?? 0) <= 64, (detail.rarity?.utf8.count ?? 0) <= 128, (detail.types?.count ?? 0) <= 8,
              detail.types?.allSatisfy({ $0.utf8.count <= 64 }) != false,
              (detail.variants_detailed?.count ?? 0) <= 100,
              detail.variants_detailed?.allSatisfy({ variant in
                  (variant.variantId?.utf8.count ?? 0) <= 128 && variant.type.utf8.count <= 64 &&
                  (variant.stamp?.count ?? 0) <= 8 && variant.stamp?.allSatisfy({ $0.utf8.count <= 80 }) != false
              }) != false else {
            throw ScannerError("catalogue", "Invalid or oversized card metadata.")
        }
        return detail
    }
    func prices(_ id: String) async throws -> CardMarketPrices {
        try validate(id)
        let detail = try JSONDecoder().decode(PricingDetail.self, from: await get(path: "cards/" + id, limit: 256 * 1024))
        guard detail.id == id, (detail.variants_detailed?.count ?? 0) <= 100 else {
            throw ScannerError("catalogue", "Invalid pricing metadata.")
        }
        return detail.quotes()
    }
    private func validate(_ id: String) throws {
        guard !id.isEmpty, id.utf8.count <= 100, id.utf8.allSatisfy({ (48...57).contains($0) || (65...90).contains($0) || (97...122).contains($0) || $0 == 43 || $0 == 45 || $0 == 46 }) else {
            throw ScannerError("catalogue", "Invalid catalogue card identifier.")
        }
    }
    private func get(path: String, limit: Int) async throws -> Data {
        let url = URL(string: "https://api.tcgdex.net/v2/" + language.rawValue + "/" + path)!
        if let load {
            let data = try await load(url, limit)
            guard data.count <= limit else { throw ScannerError("catalogue", "TCGdex response exceeds the size limit.") }
            return data
        }
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
