import Foundation

/// Printed English SWU footer identifiers; no catalogue names or scan-specific IDs.
public enum StarWarsPrintedText {
    public struct Identifier: Hashable, Sendable { public var set: String; public var number: String; public var total: Int? }
    public static func identifiers(_ lines: [String]) -> Set<Identifier> {
        let text = lines.joined(separator: " ").uppercased()
        let pattern = #"\b([A-Z][A-Z0-9]{2,3})\s*[•·.\-]?\s*EN\b[^0-9/]{0,12}\b([0-9]{1,3})(?:\s*/\s*([0-9]{3})\b|(?!\s*/)\b)"#
        let regex = try! NSRegularExpression(pattern: pattern)
        return Set(regex.matches(in: text, range: NSRange(text.startIndex..., in: text)).compactMap { m in
            guard let a = Range(m.range(at: 1), in: text), let b = Range(m.range(at: 2), in: text),
                  let number = Int(text[b]) else { return nil }
            let total = Range(m.range(at: 3), in: text).flatMap { Int(text[$0]) }
            return Identifier(set: String(text[a]), number: String(number), total: total)
        })
    }
    public static func isLeaderUnit(_ words: [String]) -> Bool {
        let text = words.joined(separator: " ").uppercased()
        return text.contains("LEADER UNIT") && text.contains("OVERWHELM") && text.contains("SENTINEL")
    }
    public static func isCard(_ words: [String]) -> Bool {
        let text = words.joined(separator: " ").uppercased()
        let copyright = text.contains("LFL") && text.contains("FFG")
        return copyright && (!identifiers(words).isEmpty || (text.contains("TOKEN") && text.contains("EN")))
    }
}
