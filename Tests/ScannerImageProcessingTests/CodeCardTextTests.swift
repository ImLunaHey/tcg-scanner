import Foundation
import Testing
@testable import ScannerImageProcessing

@Test func codeCardClassificationUsesPrintedBrandingNotCardNumbers() {
    #expect(CodeCardText.matches(["Pokémon TCG Live", "Redeem your rewards"]))
    #expect(CodeCardText.matches(["Use this code at", "pokemon.com/redeem"]))
    #expect(CodeCardText.matches(["www.pokemon.com/tcgo"]))
    #expect(!CodeCardText.matches(["Pokemon", "ENERGY", "©2022 Nintendo"]))
    #expect(!CodeCardText.matches(["Code", "Play online", "Other game"]))
    #expect(!CodeCardText.matches(["Pokémon", "TCG"]))
    #expect(!CodeCardText.matches([]))
}
@Test func savedCodeCardsAreClassifiedWithoutRecordingRedemptionCodes() throws {
    guard let path = ProcessInfo.processInfo.environment["SCANNER_CODE_CARD_FIXTURES"] else { return }
    struct Item: Decodable { let number: Int; let sequence: Int; let path: String }
    let items = try JSONDecoder().decode([Item].self, from: Data(contentsOf: URL(fileURLWithPath: path)))
    for item in items {
        let result = try CardProcessor.uprightRotation(Data(contentsOf: URL(fileURLWithPath: item.path)))
        print("CODE_CARD", item.number, item.sequence, result.basis, result.verified)
        #expect(["pokemon-code-text", "pokemon-code-url"].contains(result.basis))
    }
}

@Test func savedRawCodeCardsRetainClassificationThroughAutomaticCropping() throws {
    guard let root = ProcessInfo.processInfo.environment["SCANNER_RAW_CODE_CARD_FIXTURES"] else { return }
    for sequence in [31,32,39,40,41,42] {
        let file = URL(fileURLWithPath: root).appendingPathComponent(String(format: "C-%03d.png", sequence))
        let result = try CardProcessor.analyze(Data(contentsOf: file))
        print("RAW_CODE_CARD", sequence, result.orientationBasis ?? "unknown", result.quarterTurns)
        #expect(["pokemon-code-text", "pokemon-code-url"].contains(result.orientationBasis ?? "") && result.orientationVerified)
    }
}

@Test func redemptionURLRequiresExactPublicHostAndPath() {
    for text in ["pokemon.com/redeem", "Visit https://www.pokemon.com/redeem.", "pokemon . com / redeem", "www.pokemon.com/tcgo"] {
        #expect(CodeCardText.hasRedemptionURL([text]))
    }
    for text in ["notpokemon.com/redeem", "pokemon.com/redeemer", "pokemon.com/redeem/other", "pokemon.com.evil/redeem", "example.com/pokemon.com/redeem", "Redeem your rewards", "Pokémon TCG Live"] {
        #expect(!CodeCardText.hasRedemptionURL([text]))
    }
}

@Test func savedSingleRedemptionSideClassifiesWithoutReadableCompanion() throws {
    guard let path = ProcessInfo.processInfo.environment["SCANNER_REDEMPTION_URL_IMAGE"] else { return }
    let result = try CardProcessor.analyze(Data(contentsOf: URL(fileURLWithPath: path)))
    #expect(result.orientationBasis == "pokemon-code-url")
    #expect(result.orientationVerified)
}
