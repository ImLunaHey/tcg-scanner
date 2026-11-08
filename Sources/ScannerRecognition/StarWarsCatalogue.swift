import Foundation
import ScannerAPI

struct StarWarsCard: Decodable, Sendable {
    var Set: String; var Number: String; var Name: String; var Subtitle: String?
    var `Type`: String; var Rarity: String?; var VariantType: String?; var FrontArt: String?
}
struct StarWarsSet: Decodable, Sendable {
    var setId: String; var fullName: String; var numberCards: Int
}

/// Fixed public GETs, bounded responses, no redirects or image uploads.
actor StarWarsCatalogue {
    private struct Envelope: Decodable { var data: [StarWarsCard] }
    private let session: URLSession
    private let load: (@Sendable (URL) async throws -> Data)?
    private var setsCache: (Date, [StarWarsSet])?
    private var cardCache: [String: (Date, [StarWarsCard])] = [:]
    init(load: (@Sendable (URL) async throws -> Data)? = nil) {
        self.load = load
        let c = URLSessionConfiguration.ephemeral
        c.timeoutIntervalForRequest = 15; c.timeoutIntervalForResource = 25
        c.httpCookieStorage = nil; c.urlCache = nil; c.httpMaximumConnectionsPerHost = 2
        session = URLSession(configuration: c, delegate: StarWarsNoRedirects(), delegateQueue: nil)
    }
    func sets() async throws -> [StarWarsSet] {
        if let cache = setsCache, Date().timeIntervalSince(cache.0) < 86400 { return cache.1 }
        let result = try JSONDecoder().decode([StarWarsSet].self, from: await get("sets"))
        guard result.count < 500, result.allSatisfy({ $0.setId.count <= 16 && $0.fullName.count <= 128 && $0.numberCards > 0 }) else { throw ScannerError("catalogue", "Invalid SWU set metadata.") }
        setsCache = (Date(), result); return result
    }
    func cards(set: String) async throws -> [StarWarsCard] {
        guard set.range(of: #"^[A-Z0-9]{3,8}$"#, options: .regularExpression) != nil else { throw ScannerError("catalogue", "Invalid SWU set identifier.") }
        if let cache = cardCache[set], Date().timeIntervalSince(cache.0) < 86400 { return cache.1 }
        let cards = try JSONDecoder().decode(Envelope.self, from: await get("cards/" + set.lowercased())).data
        guard cards.count <= 5000, cards.allSatisfy({ $0.Set == set && $0.Number.count <= 16 && !$0.Name.isEmpty && $0.Name.count <= 160 && ($0.Subtitle?.count ?? 0) <= 160 && $0.Type.count <= 80 }) else { throw ScannerError("catalogue", "Invalid SWU card metadata.") }
        if cardCache.count >= 32 { cardCache.removeAll() }
        cardCache[set] = (Date(), cards); return cards
    }
    private func get(_ path: String) async throws -> Data {
        let url = URL(string: "https://api.swu-db.com/" + path)!, limit = 8 * 1024 * 1024
        if let load { let data = try await load(url); guard data.count <= limit else { throw ScannerError("catalogue", "SWU response too large.") }; return data }
        let (stream, response) = try await session.bytes(from: url)
        guard let http = response as? HTTPURLResponse, http.statusCode == 200, http.url == url,
              response.expectedContentLength <= limit else { throw ScannerError("catalogueUnavailable", "SWU catalogue request failed.") }
        var data = Data()
        for try await byte in stream {
            guard data.count < limit else { throw ScannerError("catalogue", "SWU response too large.") }
            data.append(byte)
        }
        return data
    }
}
private final class StarWarsNoRedirects: NSObject, URLSessionTaskDelegate, Sendable {
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest) async -> URLRequest? { nil }
}
