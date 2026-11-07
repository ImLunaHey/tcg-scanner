import Foundation
import ScannerAPI

public actor StarWarsRecognizer: CardRecognizing {
    private let catalogue: StarWarsCatalogue
    public init() { catalogue = StarWarsCatalogue() }
    init(catalogue: StarWarsCatalogue) { self.catalogue = catalogue }
    public func recognize(_ data: Data, language: CardLanguage) async throws -> RecognitionEvidence {
        guard language == .english else { throw ScannerError("unsupportedLanguage", "Star Wars: Unlimited matching supports English cards only.") }
        let task = Task.detached(priority: .utility) { try CardOCR.read(data) }
        let readings = try await withTaskCancellationHandler { try await task.value } onCancel: { task.cancel() }
        let sets = try await catalogue.sets()
        var results: [(CardOCR.Reading, [CardCandidate])] = []
        for reading in readings {
            try Task.checkCancellation()
            let identifiers = StarWarsPrintedText.identifiers(reading.lines.filter { $0.confidence >= 0.5 }.map(\.text))
            guard identifiers.count == 1, let id = identifiers.first,
                  let set = sets.first(where: { $0.setId == id.set && (id.total == nil || $0.numberCards == id.total) }) else { continue }
            let cards = try await catalogue.cards(set: set.setId)
            results.append((reading, StarWarsMatcher.match(lines: reading.lines, id: id, set: set, cards: cards)))
        }
        let matches = results.filter { !$0.1.isEmpty }
        guard Set(matches.flatMap { $0.1.map(\.id) }).count == 1, let best = matches.first else {
            return RecognitionEvidence(lines: readings.max(by: { $0.lines.count < $1.lines.count })?.lines ?? [], candidates: [],
                warnings: ["No unique SWU printing matched the printed set, number, total and name. Tokens or unreadable identifiers remain unresolved."])
        }
        return RecognitionEvidence(lines: best.0.lines, candidates: best.1,
            warnings: ["Catalogue match requires review. Foil finish and condition are not assessed for Star Wars: Unlimited."], orientation: best.0.rotation)
    }
}
enum StarWarsMatcher {
    static func oneSubstitution(_ a: String, _ b: String) -> Bool {
        a.count >= 6 && a.count == b.count && zip(a,b).filter { $0 != $1 }.count == 1
    }
    static func match(lines: [RecognizedLine], id: StarWarsPrintedText.Identifier, set: StarWarsSet, cards: [StarWarsCard]) -> [CardCandidate] {
        guard id.set == set.setId, (id.total == nil || id.total == set.numberCards) else { return [] }
        let words = lines.filter { $0.confidence >= 0.5 }.map { CardMatcher.normalized($0.text) }
        let matches = cards.filter { card in
            card.Set == id.set && Int(card.Number) == Int(id.number) && words.contains { $0 == CardMatcher.normalized(card.Name) ||
                (card.Subtitle.map { subtitle in words.contains(CardMatcher.normalized(subtitle)) } == true &&
                 oneSubstitution($0, CardMatcher.normalized(card.Name))) }
        }
        guard matches.count == 1, let card = matches.first else { return [] }
        let key = card.Set + "-" + card.Number
        var result = CardCandidate(id: "swu-db:" + key, name: card.Name + (card.Subtitle.map { " — " + $0 } ?? ""),
            localID: card.Number, setID: card.Set, setName: set.fullName, officialSetCount: set.numberCards,
            evidence: [words.contains(CardMatcher.normalized(card.Name)) ? "Exact printed name" : "Name with one OCR substitution and exact subtitle", "Printed set code " + id.set, "Collector number " + card.Number], category: card.Type)
        if let total = id.total { result.evidence.append("Printed set total " + String(total)) }
        result.provider = "swu-db"; result.catalogueID = key; result.rarities = card.Rarity.map { [$0] }
        return [result]
    }
}
