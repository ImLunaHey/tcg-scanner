import Foundation
import Testing
@testable import ScannerImageProcessing

@Test func energyOrientationNeedsHeadingAndMatchingFooterPositions() {
    typealias Line = EnergyTextOrientation.Line
    let heading = Line(text: "ENERGY", confidence: 1, center: CGPoint(x: 0.8,y: 0.04), turns: 2)
    let footer = Line(text: "©2022 Pokémon / Nintendo / Creatures / GAME FREAK", confidence: 1, center: CGPoint(x: 0.5,y: 0.97), turns: 2)
    #expect(EnergyTextOrientation.rotation(lines: [heading,footer]) == 2)
    #expect(EnergyTextOrientation.rotation(lines: [heading]) == nil)
    #expect(EnergyTextOrientation.rotation(lines: [footer]) == nil)
    let body = Line(text: "ENERGY", confidence: 1, center: CGPoint(x: 0.3,y: 0.5), turns: 2)
    #expect(EnergyTextOrientation.rotation(lines: [body,footer]) == nil)
}
@Test func savedLowTextEnergyOrientation() throws {
    guard let path = ProcessInfo.processInfo.environment["SCANNER_ENERGY_ORIENTATION_IMAGE"] else { return }
    let data = try Data(contentsOf: URL(fileURLWithPath: path))
    let result = try CardProcessor.uprightRotation(data)
    print("ENERGY_ORIENTATION",result)
    #expect(result.turns == 2 && result.verified && result.basis == "text")
}
