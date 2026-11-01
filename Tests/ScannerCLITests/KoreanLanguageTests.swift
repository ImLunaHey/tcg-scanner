import Testing
@testable import ScannerCLI

@Test func scanCLIRequiresAnExplicitSupportedCardLanguage() throws {
    let korean = try Scanner.parseAsRoot(["scan", "--prepare-cards", "--card-language", "ko"]) as! Scan
    #expect(korean.prepareCards && korean.cardLanguage == "ko")
    let english = try Scanner.parseAsRoot(["scan", "--prepare-cards"]) as! Scan
    #expect(english.cardLanguage == "en")
    for language in ["ja", "Korean", "KO", ""] {
        #expect(throws: (any Error).self) {
            try Scanner.parseAsRoot(["scan", "--prepare-cards", "--card-language", language])
        }
    }
    #expect(throws: (any Error).self) { try Scanner.parseAsRoot(["scan", "--card-language", "ko"]) }
}
