import Foundation
import ScannerAPI
import Testing
@testable import ScannerRecognition

private actor LanguageCatalogueFixture {
    private(set) var requests: [URL] = []
    func read(_ url: URL, _ limit: Int) throws -> Data {
        requests.append(url)
        switch url.path {
        case "/v2/en/cards":
            return Data(#"[{"id":"SV4K-001","localId":"001","name":"Pansage"}]"#.utf8)
        case "/v2/ko/cards":
            return Data(#"[{"id":"SV4K-001","localId":"001","name":"야나프"}]"#.utf8)
        case "/v2/ko/cards/SM1+-001":
            return Data(#"{"id":"SM1+-001","localId":"001","name":"나몰빼미","set":{"id":"SM1+","name":"썬&문","cardCount":{"official":51}}}"#.utf8)
        default:
            throw ScannerError("catalogueUnavailable", "No entry in the requested language.")
        }
    }
}

@Test func koreanAndEnglishCataloguesUseIndependentURLsAndCaches() async throws {
    let fixture = LanguageCatalogueFixture()
    let english = TCGdexClient(language: .english, load: { try await fixture.read($0, $1) })
    let korean = TCGdexClient(language: .korean, load: { try await fixture.read($0, $1) })
    #expect(try await english.catalogue().first?.name == "Pansage")
    #expect(try await korean.catalogue().first?.name == "야나프")
    #expect(try await korean.catalogue().first?.name == "야나프")
    #expect(try await english.catalogue().first?.name == "Pansage")
    let requests = await fixture.requests
    #expect(requests.map(\.absoluteString) == ["https://api.tcgdex.net/v2/en/cards", "https://api.tcgdex.net/v2/ko/cards"])
}

@Test func missingKoreanPrintingNeverRequestsAnEnglishFallback() async throws {
    let fixture = LanguageCatalogueFixture()
    let korean = TCGdexClient(language: .korean, load: { try await fixture.read($0, $1) })
    await #expect(throws: ScannerError.self) { try await korean.detail("unknown-001") }
    #expect(await fixture.requests.map(\.path) == ["/v2/ko/cards/unknown-001"])
    #expect(try await korean.detail("SM1+-001").set.id == "SM1+")
    for bad in ["../en/cards", "SV4K-001?language=en", "SV4K-001#en", "%2e%2e"] {
        await #expect(throws: ScannerError.self) { try await korean.detail(bad) }
    }
    #expect(await fixture.requests.count == 2)
}

@Test func koreanMatchingKeepsHangulAndRequiresIndependentPrintedNumbers() {
    let cards = [CatalogueCard(id: "SV4K-001", localId: "001", name: "야나프"),
        CatalogueCard(id: "test-010", localId: "010", name: "뮤"),
        CatalogueCard(id: "test-011", localId: "011", name: "뮤 ex")]
    let title = RecognizedLine(text: "뮤", x: 0.15, y: 0.88, height: 0.04)
    let number = RecognizedLine(text: "010/066", x: 0.08, y: 0.035, height: 0.02)
    #expect(CardMatcher.normalized("야나프") == "야나프")
    let result = CardMatcher.rank(lines: [title, number], catalogue: cards, language: .korean)
    #expect(result.map(\.card.id) == ["test-010"])
    #expect(result.first?.totals == [66])
    #expect(result.first?.evidence.contains("Collector number 010") == true)
    #expect(CardMatcher.rank(lines: [title], catalogue: cards, language: .korean).isEmpty)
    #expect(CardMatcher.rank(lines: [title, RecognizedLine(text: "011/066", y: 0.03, height: 0.02)], catalogue: cards, language: .korean).isEmpty)
    #expect(CardMatcher.rank(lines: [RecognizedLine(text: "야나프", y: 0.88, height: 0.04)], catalogue: cards, language: .korean).isEmpty)
    #expect(CardMatcher.rank(lines: [RecognizedLine(text: "뮤츠", y: 0.88, height: 0.04), number], catalogue: cards, language: .korean).isEmpty)
    #expect(CardMatcher.rank(lines: [RecognizedLine(text: "Mew", y: 0.88, height: 0.04), number], catalogue: cards, language: .korean).isEmpty)
}

@Test func koreanOCRUsesAnExplicitSupportedLanguageConfiguration() throws {
    let korean = try CardOCR.request(language: .korean)
    #expect(korean.recognitionLanguages == ["ko-KR", "en-US"])
    #expect(!korean.automaticallyDetectsLanguage && !korean.usesLanguageCorrection)
    let english = try CardOCR.request(language: .english)
    #expect(english.recognitionLanguages == ["en-US"])
}

@Test func koreanMatchingRejectsConflictingSetsAndDroppedCardClassSuffixes() {
    let card = CatalogueCard(id: "SV4K-001", localId: "001", name: "야나프")
    let title = RecognizedLine(text: "야나프", y: 0.88, height: 0.04)
    let number = RecognizedLine(text: "001/066", y: 0.03, height: 0.02)
    let same = RecognizedLine(text: "sv4K", y: 0.03, height: 0.02)
    let other = RecognizedLine(text: "SV5K", y: 0.03, height: 0.02)
    #expect(CardMatcher.rank(lines: [title, number, same], catalogue: [card], language: .korean).count == 1)
    #expect(CardMatcher.rank(lines: [title, number, other], catalogue: [card], language: .korean).isEmpty)
    #expect(CardMatcher.rank(lines: [title, number, same, other], catalogue: [card], language: .korean).isEmpty)
    for suffix in ["ex", "GX", "V", "VMAX", "VSTAR", "BREAK"] {
        let ordinary = CatalogueCard(id: "SV4K-001", localId: "001", name: "뮤")
        let special = CatalogueCard(id: "SV4K-001", localId: "001", name: "뮤 " + suffix)
        let printed = RecognizedLine(text: "뮤 " + suffix, y: 0.88, height: 0.04)
        #expect(CardMatcher.rank(lines: [printed, number], catalogue: [ordinary], language: .korean).isEmpty)
        #expect(CardMatcher.rank(lines: [printed, number], catalogue: [special], language: .korean).count == 1)
        let split = [RecognizedLine(text: "뮤", y: 0.88, height: 0.04), RecognizedLine(text: suffix, y: 0.88, height: 0.04), number]
        #expect(CardMatcher.rank(lines: split, catalogue: [ordinary], language: .korean).isEmpty)
    }
}
