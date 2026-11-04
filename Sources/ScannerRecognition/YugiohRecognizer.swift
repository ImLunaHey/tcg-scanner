import Foundation
import ScannerAPI

public actor YugiohRecognizer: CardRecognizing {
    private let catalogue: YugiohCatalogue
    public init() { catalogue = YugiohCatalogue() }
    init(catalogue: YugiohCatalogue) { self.catalogue = catalogue }

    public func recognize(_ data: Data, language: CardLanguage) async throws -> RecognitionEvidence {
        guard language == .english else { throw ScannerError("unsupportedLanguage", "Yu-Gi-Oh! catalogue matching currently supports English cards only.") }
        let task = Task.detached(priority: .utility) { try CardOCR.read(data, language: .english) }
        let readings = try await withTaskCancellationHandler { try await task.value } onCancel: { task.cancel() }
        let snapshot = try await catalogue.catalogue()
        try Task.checkCancellation()
        let matches = readings.map { ($0, YugiohMatcher.match(lines: $0.lines, cards: snapshot.cards)) }
        let best = matches.max { $0.1.score < $1.1.score }!
        var warnings = best.1.warnings
        if snapshot.stale { warnings.append("Catalogue refresh failed; using the previously cached catalogue.") }
        if let card = best.1.card {
            do { try await catalogue.cacheReference(card) }
            catch is CancellationError { throw CancellationError() }
            catch { warnings.append("Reference artwork could not be cached. Text matching is retained.") }
        }
        try Task.checkCancellation()
        return RecognitionEvidence(lines: best.0.lines, candidates: best.1.candidates, warnings: warnings, orientation: best.0.rotation)
    }
}

enum YugiohMatcher {
    struct Match { var card: YugiohCard?; var candidates: [CardCandidate] = []; var score = 0; var warnings: [String] = [] }
    static func normalized(_ text: String) -> String {
        text.folding(options: [.caseInsensitive, .diacriticInsensitive], locale: Locale(identifier: "en_US_POSIX")).filter { $0.isLetter || $0.isNumber }
    }
    static func captures(_ pattern: String, _ text: String) -> [String] {
        guard let regex = try? NSRegularExpression(pattern: pattern) else { return [] }
        return regex.matches(in: text, range: NSRange(text.startIndex..., in: text)).compactMap {
            Range($0.range, in: text).map { String(text[$0]) }
        }
    }
    static func match(lines: [RecognizedLine], cards: [YugiohCard]) -> Match {
        let lines = lines.filter { $0.confidence >= 0.5 }
        let titles = Set(lines.filter { $0.y > 0.78 }.map { normalized($0.text) }.filter { $0.count >= 4 })
        let passcodes = Set(lines.filter { $0.y < 0.15 }.flatMap { captures(#"\b[0-9]{8}\b"#, $0.text) }.compactMap(Int.init))
        let codes = Set(lines.filter { $0.y < 0.5 }.flatMap {
            captures(#"\b[A-Z0-9]{2,6}\s*[-‐‑–]\s*(?:EN|AE)?[0-9]{2,4}\b"#, $0.text.uppercased())
        }.map { $0.replacingOccurrences(of: #"\s"#, with: "", options: .regularExpression).replacingOccurrences(of: #"[‐‑–]"#, with: "-", options: .regularExpression) })
        let byName = cards.filter { titles.contains(normalized($0.name)) }
        let byCode = cards.filter { passcodes.contains($0.id) }
        // Contradictory independently read fields must never silently override one another.
        if passcodes.count > 1 || byName.count > 1 || byCode.count > 1 ||
            (!byName.isEmpty && !byCode.isEmpty && byName[0].id != byCode[0].id) {
            return Match(warnings: ["Conflicting name or passcode evidence; no card was selected."])
        }
        guard let card = byCode.first ?? byName.first else {
            return Match(warnings: ["No exact English card name or printed passcode matched the local catalogue."])
        }
        if !passcodes.isEmpty && !passcodes.contains(card.id) {
            return Match(warnings: ["Printed passcode conflicts with the name match; no card was selected."])
        }
        let matched = (card.card_sets ?? []).filter { codes.contains($0.set_code.uppercased()) }
        let groups = Dictionary(grouping: matched, by: { $0.set_code.uppercased() })
        guard groups.count <= 1 else { return Match(warnings: ["Multiple printed set codes matched; review the saved image."]) }
        var evidence: [String] = []
        if passcodes.contains(card.id) { evidence.append("Printed passcode " + String(format: "%08d", card.id)) }
        if !byName.isEmpty { evidence.append("Exact printed name " + card.name) }
        var warnings = ["Catalogue rarity describes possible printings, not a measured foil finish or condition grade."]
        let code = groups.keys.first ?? ""
        let variants = groups[code] ?? []
        if !code.isEmpty { evidence.append("Printed set code " + code) }
        else { warnings.append(codes.isEmpty ? "Printed set code was not readable; printing is unresolved." : "Printed set code is not listed for this card; printing is unresolved.") }
        let names = Set(variants.map(\.set_name)).sorted()
        var candidate = CardCandidate(id: "ygoprodeck:\(card.id):\(code.isEmpty ? "unresolved" : code)", name: card.name,
            localID: code, setID: code, setName: names.isEmpty ? "Printing not identified" : names.joined(separator: " / "),
            evidence: evidence, category: card.type)
        candidate.provider = "ygoprodeck"; candidate.catalogueID = String(card.id)
        candidate.rarities = Array(Set(variants.map(\.set_rarity))).sorted()
        if (candidate.rarities?.count ?? 0) > 1 { warnings.append("This set code has several catalogue rarities; the physical variant remains unresolved.") }
        return Match(card: card, candidates: [candidate], score: evidence.count, warnings: warnings)
    }
}
