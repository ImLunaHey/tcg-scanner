import Foundation
import Testing
import ScannerAPI
@testable import ScannerImageProcessing

/// Opt-in local evidence: never feeds hardware, uploads images or requires private
/// scan fixtures in ordinary CI. Includes backs, fronts, and all quarter turns.
@Test(.enabled(if: ProcessInfo.processInfo.environment["SCANNER_GAME_FIXTURE"] != nil))
func savedGameBacksDetectAcrossRotations() throws {
    let root = try #require(ProcessInfo.processInfo.environment["SCANNER_GAME_FIXTURE"])
    let game = ProcessInfo.processInfo.environment["SCANNER_GAME_EXPECTED"] ?? "yugioh"
    let count = Int(ProcessInfo.processInfo.environment["SCANNER_GAME_COUNT"] ?? "8")!
    for sequence in 1...count {
        let data = try Data(contentsOf: URL(fileURLWithPath: root).appendingPathComponent(String(format: "C-%03d.png", sequence)))
        let plan = try CardProcessor.analyze(data)
        let cropped = try ImageProcessor.render(data, request: ImageEditRequest(jobID: UUID(), sequence: sequence, crop: plan.crop, quarterTurns: plan.quarterTurns, deskewDegrees: plan.deskewDegrees))
        print("GAME FIXTURE", sequence, plan.orientationBasis ?? "nil", plan.quarterTurns)
        #expect(plan.orientationVerified)
        if sequence.isMultiple(of: 2) {
            #expect(plan.orientationBasis == "text")
        } else {
            #expect(plan.orientationBasis == "\(game)-back-pattern")
            for turn in 0..<4 {
                let rotated = try ImageProcessor.render(cropped.data, request: ImageEditRequest(jobID: UUID(), sequence: sequence, quarterTurns: turn))
                let result = try CardProcessor.uprightRotation(rotated.data)
                #expect(result.basis == "\(game)-back-pattern")
                #expect(result.turns == (4 - turn) % 4)
            }
        }
    }
}

@Test func tradingLogoAllowsOneOCRSubstitutionButRejectsUnrelatedText() {
    #expect(YugiohBackOrientation.resemblesTradingLogo("TRADING CAR D GAME"))
    #expect(YugiohBackOrientation.resemblesTradingLogo("TRADING GARDGAME"))
    #expect(!YugiohBackOrientation.resemblesTradingLogo("KONAMI"))
    #expect(!YugiohBackOrientation.resemblesTradingLogo("TRADING CARD SHOP"))
    #expect(!YugiohBackOrientation.resemblesTradingLogo("Pokemon"))
}
