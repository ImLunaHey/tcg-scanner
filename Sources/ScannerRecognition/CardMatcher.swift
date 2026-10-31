import Foundation
import ScannerAPI

/// Evidence ranking, not a calibrated probability. Name alone never confirms a card.
enum CardMatcher {
    struct Ranked: Sendable {
        var card: CatalogueCard
        var score: Int
        var evidence: [String]
        var totals: Set<Int>
    }
    static func normalized(_ text: String) -> String {
        text.folding(options: [.diacriticInsensitive, .caseInsensitive], locale: Locale(identifier: "en_US_POSIX"))
            .unicodeScalars.map { CharacterSet.alphanumerics.contains($0) ? String($0) : " " }
            .joined().split(separator: " ").joined(separator: " ")
    }
    static func number(_ text: String) -> String {
        let value = normalized(text).replacingOccurrences(of: " ", with: "").uppercased()
        // Preserve promo/subset prefixes while ignoring numeric zero padding.
        let prefix = value.prefix(while: { !$0.isNumber })
        let suffix = value.dropFirst(prefix.count)
        return String(prefix) + (Int(suffix).map(String.init) ?? String(suffix))
    }
    static func rank(lines: [RecognizedLine], catalogue: [CatalogueCard]) -> [Ranked] {
        let readable = lines.filter { $0.confidence >= 0.35 }
        let titles = readable.filter { $0.y + $0.height > 0.65 }.map { " " + normalized($0.text) + " " }
        let footers = readable.filter { $0.y < 0.30 }.map(\.text)
        let fraction = try! NSRegularExpression(pattern: #"(?i)(?<![A-Z0-9])([A-Z]{0,5}[0-9]{1,4})\s*/\s*([A-Z]{0,5}[0-9]{1,4})(?![A-Z0-9])"#)
        var numbers: Set<String> = []
        var totalsByNumber: [String: Set<Int>] = [:]
        for text in footers {
            for match in fraction.matches(in: text, range: NSRange(text.startIndex..., in: text)) {
                guard let local = Range(match.range(at: 1), in: text), let total = Range(match.range(at: 2), in: text) else { continue }
                let key = number(String(text[local])); numbers.insert(key)
                if let count = Int(text[total]) { totalsByNumber[key, default: []].insert(count) }
            }
        }
        // Standalone promo numbers are useful, ordinary standalone digits are too ambiguous.
        let promo = try! NSRegularExpression(pattern: #"(?i)\b(?:SWSH|SM|XY|BW|SVP|DP|HGSS)\d{1,4}\b"#)
        for text in footers {
            for match in promo.matches(in: text, range: NSRange(text.startIndex..., in: text)) {
                if let range = Range(match.range, in: text) { numbers.insert(number(String(text[range]))) }
            }
        }
        return catalogue.compactMap { card -> Ranked? in
            let name = normalized(card.name)
            guard name.count >= 3, titles.contains(where: { $0.contains(" " + name + " ") }) else { return nil }
            let local = number(card.localId)
            let numbered = numbers.contains(local)
            // A legible conflicting collector number rules out this printing.
            guard numbers.isEmpty || numbered else { return nil }
            return Ranked(card: card, score: (numbered ? 100 : 0) + name.count,
                          evidence: numbered ? ["Name in title area", "Collector number " + card.localId] : ["Name in title area only; printing is ambiguous"],
                          totals: totalsByNumber[local] ?? [])
        }.sorted { $0.score != $1.score ? $0.score > $1.score : $0.card.id < $1.card.id }
    }
}
