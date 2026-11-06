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
        #expect(result.basis == "pokemon-code-text")
    }
}
