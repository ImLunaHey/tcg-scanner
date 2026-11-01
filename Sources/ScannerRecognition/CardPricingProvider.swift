import Foundation
import ScannerAPI

public protocol CardPricingProviding: Sendable {
    func prices(cardID: String) async throws -> CardMarketPrices
    func sterlingRate() async throws -> EuroSterlingRate
}

/// Public metadata GETs only. No photographs, OCR, grades or batch data leave the Mac.
public actor CardPricingProvider: CardPricingProviding {
    private let catalogue = TCGdexClient()
    public init() {}
    public func prices(cardID: String) async throws -> CardMarketPrices {
        try await catalogue.prices(cardID)
    }
    public func sterlingRate() async throws -> EuroSterlingRate {
        let url = URL(string: "https://www.ecb.europa.eu/stats/eurofxref/eurofxref-daily.xml")!
        let configuration = URLSessionConfiguration.ephemeral
        configuration.timeoutIntervalForRequest = 10
        configuration.timeoutIntervalForResource = 15
        configuration.httpCookieStorage = nil
        let session = URLSession(configuration: configuration, delegate: PricingNoRedirects(), delegateQueue: nil)
        defer { session.invalidateAndCancel() }
        let (stream, response) = try await session.bytes(from: url)
        guard let http = response as? HTTPURLResponse, http.statusCode == 200, http.url == url,
              response.expectedContentLength <= 64 * 1024 else { throw ScannerError("pricingUnavailable", "ECB exchange rate is unavailable.") }
        var data = Data()
        for try await byte in stream {
            guard data.count < 64 * 1024 else { throw ScannerError("pricingUnavailable", "Oversized exchange rate response.") }
            data.append(byte)
        }
        return try Self.decodeRate(data)
    }
    static func decodeRate(_ data: Data) throws -> EuroSterlingRate {
        let delegate = RateParser()
        let parser = XMLParser(data: data)
        parser.shouldResolveExternalEntities = false
        parser.delegate = delegate
        guard parser.parse(), let rate = delegate.rate, rate > 0, rate < 10,
              let date = delegate.date, date.range(of: #"^\d{4}-\d{2}-\d{2}$"#, options: .regularExpression) != nil,
              let timestamp = ISO8601DateFormatter().date(from: date + "T00:00:00Z"),
              timestamp <= Date().addingTimeInterval(86400), timestamp > Date().addingTimeInterval(-14 * 86400) else {
            throw ScannerError("pricingUnavailable", "Invalid or outdated ECB exchange rate.")
        }
        return EuroSterlingRate(poundsPerEuro: rate, date: date)
    }
}

private final class RateParser: NSObject, XMLParserDelegate {
    var date: String?
    var rate: Decimal?
    func parser(_ parser: XMLParser, didStartElement elementName: String, namespaceURI: String?, qualifiedName qName: String?, attributes: [String: String]) {
        if elementName == "Cube", let time = attributes["time"] { date = time }
        if elementName == "Cube", attributes["currency"] == "GBP", let value = attributes["rate"] {
            rate = Decimal(string: value, locale: Locale(identifier: "en_US_POSIX"))
        }
    }
}
private final class PricingNoRedirects: NSObject, URLSessionTaskDelegate, Sendable {
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest) async -> URLRequest? { nil }
}

struct PricingDetail: Decodable {
    struct Prices: Decodable { var cardmarket: Market? }
    struct Market: Decodable {
        var unit: String?
        var idProduct: Int?
        var updated: String?
        var trend: Decimal?
        var reverseTrend: Decimal?
        enum CodingKeys: String, CodingKey {
            case unit, idProduct, updated, trend
            case reverseTrend = "trend-holo"
        }
    }
    struct Variant: Decodable {
        var variantId: String?
        var type: String
        var size: String?
        var foil: String?
        var stamp: [String]?
        var subtype: String?
        var pricing: Prices?
    }
    var id: String
    var variants: [String: Bool]?
    var variants_detailed: [Variant]?
    var pricing: Prices?

    func quotes() -> CardMarketPrices {
        let mapping: [String: CardFinish] = ["normal": .normal, "holo": .holo, "reverse": .reverseHolo]
        let details = (variants_detailed ?? []).filter {
            $0.size == "standard" && $0.subtype == nil && mapping[$0.type] != nil &&
                ($0.variantId?.utf8.count ?? 0) <= 128 && ($0.stamp?.count ?? 0) <= 8 &&
                $0.stamp?.allSatisfy({ !$0.isEmpty && $0.utf8.count <= 80 }) != false
        }
        var quotes: [MarketVariantPrice] = []
        for variant in details {
            let market = variant.pricing?.cardmarket
            // Cardmarket's base series covers the regular printing (including holo rares).
            // Its parallel foil series is the reverse printing. Never substitute series.
            var value = variant.type == "reverse" ? market?.reverseTrend : market?.trend
            if variant.type != "reverse", let product = market?.idProduct,
               details.contains(where: {
                   $0.type != "reverse" && $0.pricing?.cardmarket?.idProduct == product &&
                       ($0.type != variant.type || Set($0.stamp ?? []) != Set(variant.stamp ?? []))
               }) {
                value = nil // One market product cannot distinguish different regular finishes or stamped editions.
            }
            if market?.unit != "EUR" || value.map({ $0 <= 0 || $0 > 1_000_000 || $0.isNaN }) != false { value = nil }
            quotes.append(MarketVariantPrice(finish: mapping[variant.type]!, pattern: variant.foil,
                productID: market?.idProduct, euros: value, updated: market?.updated,
                variantID: variant.variantId, stamps: variant.stamp ?? []))
        }
        // Older responses without detailed variants: root pricing is unambiguous only
        // when a single regular printing exists. Never fall back over missing detail quotes.
        if variants_detailed == nil || variants_detailed?.isEmpty == true {
            let regular = ["normal", "holo"].filter { variants?[$0] == true }
            for kind in ["normal", "holo", "reverse"] where variants?[kind] == true {
                let market = pricing?.cardmarket
                var value = kind == "reverse" ? market?.reverseTrend : regular.count == 1 ? market?.trend : nil
                if market?.unit != "EUR" || value.map({ $0 <= 0 || $0 > 1_000_000 || $0.isNaN }) != false { value = nil }
                quotes.append(MarketVariantPrice(finish: mapping[kind]!, productID: market?.idProduct, euros: value, updated: market?.updated))
            }
        }
        return CardMarketPrices(cardID: id, variants: quotes, requiresPrintingReview: variants?["firstEdition"] == true)
    }
}
