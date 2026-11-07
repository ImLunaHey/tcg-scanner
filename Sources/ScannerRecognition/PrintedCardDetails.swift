import Foundation
import ScannerAPI

/// Transcribed printing details, never a catalogue identity or inventory ID.
enum PrintedCardDetails {
    static func read(_ lines: [RecognizedLine]) -> UncataloguedCard? {
        let footer = lines.filter { $0.confidence >= 0.7 && $0.y < 0.15 && $0.height < 0.1 }
        let codePattern = #"(?i)\b([A-Z0-9]{3})\s+EN\b"#
        let numberPattern = #"(?i)^\s*([A-Z]{0,5}[0-9]{1,4})\s*/\s*([0-9]{1,4})(?![0-9])"#
        func captures(_ pattern: String, _ text: String) -> [String]? {
            guard let regex = try? NSRegularExpression(pattern: pattern),
                  let match = regex.firstMatch(in: text, range: NSRange(text.startIndex..., in: text)) else { return nil }
            return (1..<match.numberOfRanges).compactMap { Range(match.range(at: $0), in: text).map { String(text[$0]) } }
        }
        let codes = Set(footer.compactMap { captures(codePattern, $0.text)?.first?.uppercased() })
        let fractions = Set(footer.compactMap { captures(numberPattern, $0.text)?.joined(separator: "/") })
        guard codes.count == 1, let code = codes.first, fractions.count == 1, let fraction = fractions.first else { return nil }
        let titles = lines.filter {
            $0.confidence >= 0.7 && $0.y > 0.8 && $0.height > 0.025 && $0.height < 0.12 &&
            $0.text.rangeOfCharacter(from: .letters) != nil &&
            !["basic", "stage 1", "stage 2", "stage1", "stage2", "trainer", "energy", "hp", "supporter", "item", "stadium", "pokemon tool"].contains(CardMatcher.normalized($0.text)) &&
            !$0.text.lowercased().hasPrefix("evolves from")
        }.sorted { $0.height > $1.height }
        guard let title = titles.first else { return nil }
        let name = title.text.replacingOccurrences(of: #"(?i)\s+(?:HP\s*\d+|\d+\s*HP)\s*$"#, with: "", options: .regularExpression)
        guard (2...100).contains(name.count) else { return nil }
        return UncataloguedCard(name: name, setCode: code, collectorNumber: fraction)
    }
}
