import Foundation
import Testing
import ScannerAPI
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
    for text in ["notpokemon.com/redeem", "pokemon.com/redeemer", "pokemon.com/redeem/other", "pokemon.com/redeem.html", "pokemon.com.evil/redeem", "example.com/pokemon.com/redeem", "Redeem your rewards", "Pokémon TCG Live"] {
        #expect(!CodeCardText.hasRedemptionURL([text]))
    }
}

@Test func savedSingleRedemptionSideClassifiesWithoutReadableCompanion() throws {
    guard let path = ProcessInfo.processInfo.environment["SCANNER_REDEMPTION_URL_IMAGE"] else { return }
    let result = try CardProcessor.analyze(Data(contentsOf: URL(fileURLWithPath: path)))
    #expect(result.orientationBasis == "pokemon-code-url")
    #expect(result.orientationVerified)
}

@Test func sparseLiveLogoEstablishesDirectionWithoutFourTextLines() {
    typealias Line = CodeCardOrientation.Line
    for turns in 0..<4 {
        let lines = [Line(text: "LIVE", confidence: 1, turns: turns), Line(text: "TRADING CARD GAME", confidence: 1, turns: turns)]
        #expect(CodeCardOrientation.rotation(lines: lines)?.turns == turns)
        #expect(CodeCardOrientation.rotation(lines: lines)?.basis == "pokemon-code-text")
        #expect(CodeCardOrientation.rotation(lines: [lines[0]]) == nil)
        #expect(CodeCardOrientation.rotation(lines: [lines[0], Line(text: "TRADING CARD GAME", confidence: 1, turns: (turns + 2) % 4)]) == nil)
        #expect(CodeCardOrientation.rotation(lines: [Line(text: "POKEMON.COM/REDEEM", confidence: 1, turns: turns)])?.turns == turns)
    }
}
@Test func savedLiveLogosAreUprightWithoutReadingRedemptionCodes() throws {
    guard let path = ProcessInfo.processInfo.environment["SCANNER_LIVE_LOGO_FIXTURES"] else { return }
    let paths = try JSONDecoder().decode([String].self, from: Data(contentsOf: URL(fileURLWithPath: path)))
    for path in paths {
        let data = try Data(contentsOf: URL(fileURLWithPath: path))
        for turn in 0..<4 {
            let rotated = try ImageProcessor.render(data, request: ImageEditRequest(jobID: UUID(), sequence: 1, quarterTurns: turn)).data
            let result = try CardProcessor.uprightRotation(rotated)
            #expect(result.turns == (2 - turn + 4) % 4)
            #expect(result.verified && result.basis == "pokemon-code-text")
        }
    }
}
