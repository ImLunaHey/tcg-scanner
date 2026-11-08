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
    /// Require the same printing and one identical, independently read set total.
    /// Never repair an OCR number by trimming digits to match catalogue data.
    static func agreeingFooterRanks(_ first: [Ranked], _ second: [Ranked]) -> [Ranked] {
        first.filter { match in
            match.totals.count == 1 && second.contains {
                $0.card.id == match.card.id && $0.totals == match.totals
            }
        }
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
    /// Vision can read inverted text while leaving its boxes upside down. A
    /// collector fraction near the top must not turn flavour text into a title.
    static func collectorLines(_ lines: [RecognizedLine]) -> [RecognizedLine] {
        lines.filter {
            $0.confidence >= 0.35 && $0.height <= 0.10 &&
            $0.text.range(of: #"(?i)^\s*[A-Z]{0,5}[0-9]{1,4}\s*/\s*[A-Z]{0,5}[0-9]{1,4}(?:\s*[^A-Z0-9/\s]+|\s+[O0])?\s*$"#, options: .regularExpression) != nil
        }
    }
    static func footerOrientationScore(_ lines: [RecognizedLine]) -> Int {
        collectorLines(lines).contains { $0.y < 0.30 } ? 1 : 0
    }
    static func rank(lines: [RecognizedLine], catalogue: [CatalogueCard], language: CardLanguage = .english) -> [Ranked] {
        guard !collectorLines(lines).contains(where: { $0.y > 0.65 }) else { return [] }
        let readable = lines.filter { $0.confidence >= 0.35 }
        let titles = readable.filter { $0.y + $0.height > 0.65 && $0.height < 0.15 }.map { " " + normalized($0.text) + " " }
        let footers = readable.filter { $0.y < 0.30 && $0.height <= 0.10 }.map(\.text)
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
        // Modern basic Energy prints a symbol instead of its type name.
        // Require a supported printed set code and its own collector number.
        let energyTitle = titles.contains { $0.contains(" energy ") }
        let energySets = Set(footers.compactMap { text -> String? in
            let code = normalized(text).replacingOccurrences(of: " ", with: "")
            return ["sve", "mee"].first { code == $0 || code == $0 + "en" }
        })
        let standalone = Set(footers.compactMap { text -> String? in
            // Vision may join the number and adjacent illustrator credit.
            guard let match = text.range(of: #"(?i)^\s*[0-9]{3}(?=\s*(?:$|[Il]llus\.))"#, options: .regularExpression) else { return nil }
            return number(String(text[match]))
        })
        // A limited translated catalogue must not override a clear printed set
        // or card-class suffix just because another printing shares its number.
        let setPattern = try! NSRegularExpression(pattern: #"(?i)(?<![A-Z0-9])((?:SV|SM|XY|BW|S)[0-9]{1,3}[A-Z]{0,3}\+?)(?![A-Z0-9])"#)
        let printedSets: Set<String> = language == .korean ? Set(footers.flatMap { text in
            setPattern.matches(in: text, range: NSRange(text.startIndex..., in: text)).compactMap { match in
                Range(match.range(at: 1), in: text).map { String(text[$0]).uppercased() }
            }
        }) : []
        let classSuffixes: Set<String> = ["ex", "gx", "v", "vmax", "vstar", "break"]
        let printedSuffixes = Set(titles.flatMap { $0.split(separator: " ").map(String.init).filter(classSuffixes.contains) })
        return catalogue.compactMap { card -> Ranked? in
            let name = normalized(card.name)
            guard !name.isEmpty else { return nil }
            let local = number(card.localId)
            let energyNumber = energyTitle && energySets.count == 1 && standalone.count == 1 && standalone.contains(local) && energySets.contains(card.id.split(separator: "-").dropLast().joined(separator: "-"))
            if energyTitle && !energySets.isEmpty && !energyNumber { return nil }
            let numbered = numbers.contains(local) || energyNumber
            // Valid Korean names can be only one or two Hangul syllables. Their
            // exact title still needs independent printed-number evidence.
            let hasHangul = name.unicodeScalars.contains { (0xAC00...0xD7A3).contains($0.value) }
            guard name.count >= 3 || (language == .korean && hasHangul && numbered) else { return nil }
            guard language != .korean || numbered else { return nil }
            if language == .korean {
                let set = card.id.split(separator: "-").dropLast().joined(separator: "-").uppercased()
                guard printedSets.isEmpty || printedSets == [set] else { return nil }
                let suffix = name.split(separator: " ").last.map(String.init)
                let expected: Set<String> = suffix.map { classSuffixes.contains($0) ? [$0] : [] } ?? []
                guard printedSuffixes.isEmpty || printedSuffixes == expected else { return nil }
            }
            let exactName = titles.contains { $0.contains(" " + name + " ") }
            let base = name.hasSuffix(" ex") ? String(name.dropLast(3)) : ""
            let uncertainSuffix = language == .english && numbered && !base.isEmpty && titles.contains { title in
                [" x ", " cx "].contains { title.contains(" " + base + $0) }
            }
            guard exactName || uncertainSuffix || energyNumber else { return nil }
            // A legible conflicting collector number rules out this printing.
            guard numbers.isEmpty || numbered else { return nil }
            let evidence: [String]
            if energyNumber { evidence = ["Energy title", "Printed set code " + (energySets.first ?? "").uppercased(), "Collector number " + card.localId] }
            else if uncertainSuffix { evidence = ["Name with uncertain ex suffix; collector number agrees", "Collector number " + card.localId] }
            else { evidence = numbered ? ["Name in title area", "Collector number " + card.localId] : ["Name in title area only; printing is ambiguous"] }
            return Ranked(card: card, score: (numbered ? 100 : 0) + name.count,
                          evidence: evidence, totals: totalsByNumber[local] ?? [])
        }.sorted { $0.score != $1.score ? $0.score > $1.score : $0.card.id < $1.card.id }
    }
}
